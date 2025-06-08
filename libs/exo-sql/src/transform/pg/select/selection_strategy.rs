// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    AbstractOrderBy, AbstractPredicate, Column, Database, Limit, Offset, PhysicalColumnPath,
    RelationId, Selection, TableId,
    sql::{
        predicate::ConcretePredicate, schema_object::SchemaObjectName, select::Select, table::Table,
    },
    transform::{
        join_util,
        pg::{Postgres, selection_level::SelectionLevel},
        transformer::{OrderByTransformer, PredicateTransformer},
    },
};

use super::selection_context::SelectionContext;

/// A strategy for generating a SQL query from an abstract select.
pub(crate) trait SelectionStrategy {
    /// A unique identifier for this strategy (for debugging purposes)
    fn id(&self) -> &'static str;

    /// Returns true if this strategy is suitable for the given selection context (i.e. the strategy
    /// can be used to generate a valid SQL query).
    ///
    /// Currently, we return a boolean, but we can later change to return the "cost" of the strategy
    /// based on the number of tables involved, runtime sampling of rows count, and static
    /// complexity of the generated SQL query. Then if multiple strategies are suitable, the one
    /// with the lowest cost can be used.
    ///
    /// If multiple strategies declare themselves suitable, all of them should return the same data
    /// (but not necessarily the same SQL). (TODO: Write a test for this).
    fn suitable(&self, selection_context: &SelectionContext) -> bool;

    /// Computes the SQL query for the given selection context. If a strategy
    fn to_select(&self, selection_context: SelectionContext<'_>, database: &Database) -> Select;
}

/// Compute an inner select that picks up all the columns from the given table, and applies the
/// given clauses.
#[allow(clippy::too_many_arguments)]
pub(super) fn compute_inner_select(
    table: Table,
    wildcard_table: TableId,
    predicate: ConcretePredicate,
    order_by: &Option<AbstractOrderBy>,
    limit: &Option<Limit>,
    offset: &Option<Offset>,
    selection_level: &SelectionLevel,
    transformer: &impl OrderByTransformer,
    database: &Database,
) -> Select {
    Select {
        table,
        columns: vec![Column::Star(Some(
            database.get_table(wildcard_table).name.clone(),
        ))],
        predicate,
        order_by: order_by
            .as_ref()
            .map(|ob| transformer.to_order_by(ob, selection_level, database)),
        offset: offset.clone(),
        limit: limit.clone(),
        group_by: None,
        top_level_selection: false,
    }
}

/// Compute a nested version of the given inner select, with the given selection applied.
pub(super) fn nest_subselect(
    inner_select: Select,
    selection: Selection,
    selection_level: &SelectionLevel,
    alias: (String, SchemaObjectName),
    transformer: &Postgres,
    database: &Database,
) -> Select {
    let selection_aggregate = selection.selection_aggregate(selection_level, transformer, database);

    Select {
        table: Table::SubSelect {
            select: Box::new(inner_select),
            alias: Some(alias),
        },
        columns: selection_aggregate,
        predicate: ConcretePredicate::True,
        order_by: None,
        offset: None,
        limit: None,
        group_by: None,
        top_level_selection: selection_level.is_top_level(),
    }
}

/// Compute the join and a suitable predicate for the given base table and predicate.
pub(super) fn join_info(
    base_table_id: TableId,
    predicate: &AbstractPredicate,
    predicate_column_paths: Vec<PhysicalColumnPath>,
    order_by_column_paths: Vec<PhysicalColumnPath>,
    selection_level: &SelectionLevel,
    transformer: &Postgres,
    database: &Database,
) -> (Table, ConcretePredicate) {
    let columns_paths: Vec<_> = predicate_column_paths
        .into_iter()
        .chain(order_by_column_paths)
        .collect();

    let join = join_util::compute_join(base_table_id, &columns_paths, selection_level, database);

    // If `compute_join` resulted in a join, we need to pass the selection level unchanged, so that
    // aliasing can be in sync with the join. Otherwise, we need to pass just the tail relation id
    // so that relation predicates can be applied.
    let predicate_selection_level_override = match join {
        Table::Join(_) => None,
        _ => selection_level
            .tail_relation_id()
            .map(|relation_id| SelectionLevel::Nested(vec![*relation_id])),
    };

    let relation_predicate = compute_relation_predicate(selection_level, false, database);

    let predicate_selection_level = predicate_selection_level_override
        .as_ref()
        .unwrap_or(selection_level);

    let predicate = transformer.to_predicate(predicate, predicate_selection_level, true, database);

    let predicate = ConcretePredicate::and(predicate, relation_predicate);

    (join, predicate)
}

pub(super) fn compute_relation_predicate(
    selection_level: &SelectionLevel,
    use_alias: bool,
    database: &Database,
) -> ConcretePredicate {
    let subselect_relation = match selection_level {
        SelectionLevel::TopLevel => None,
        SelectionLevel::Nested(relation_ids) => relation_ids.last().copied(),
    };

    subselect_relation
        .map(|relation_id| {
            let (relation_table_id, relation_column_pairs) = match relation_id {
                RelationId::OneToMany(relation_id) => {
                    let relation = relation_id.deref(database);
                    (relation.self_table_id, relation.column_pairs)
                }
                RelationId::ManyToOne(relation_id) => {
                    let relation = relation_id.deref(database);
                    (relation.self_table_id, relation.column_pairs)
                }
            };

            let alias = if use_alias {
                Some(selection_level.alias((relation_table_id, None), database))
            } else {
                selection_level
                    .without_last()
                    .self_referencing_table_alias(relation_table_id, database)
            };

            let foreign_table_alias =
                selection_level.self_referencing_table_alias(relation_table_id, database);

            relation_column_pairs.into_iter().fold(
                ConcretePredicate::True,
                |predicate, column_pair| {
                    ConcretePredicate::and(
                        predicate,
                        ConcretePredicate::Eq(
                            Column::physical(column_pair.self_column_id, alias.clone()),
                            Column::physical(
                                column_pair.foreign_column_id,
                                foreign_table_alias.clone(),
                            ),
                        ),
                    )
                },
            )
        })
        .unwrap_or(ConcretePredicate::True)
}
