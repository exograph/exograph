// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    sql::{predicate::ConcretePredicate, select::Select, table::Table},
    transform::{
        join_util,
        pg::{Postgres, SelectionLevel},
        transformer::{OrderByTransformer, PredicateTransformer},
    },
    AbstractOrderBy, AbstractPredicate, Column, Database, Limit, Offset, PhysicalColumnPath,
    Selection, TableId,
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
    fn to_select<'a>(
        &self,
        selection_context: SelectionContext<'_, 'a>,
        database: &'a Database,
    ) -> Select;
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
    transformer: &impl OrderByTransformer,
    database: &Database,
) -> Select {
    Select {
        table,
        columns: vec![Column::Star(Some(
            database.get_table(wildcard_table).name.clone(),
        ))],
        predicate,
        order_by: order_by.as_ref().map(|ob| transformer.to_order_by(ob)),
        offset: offset.clone(),
        limit: limit.clone(),
        group_by: None,
        top_level_selection: false,
    }
}

/// Compute a nested version of the given inner select, with the given selection applied.
pub(super) fn nest_subselect(
    inner_select: Select,
    selection: &Selection,
    selection_level: SelectionLevel,
    alias: &str,
    transformer: &Postgres,
    database: &Database,
) -> Select {
    let selection_aggregate = selection.selection_aggregate(transformer, database);

    Select {
        table: Table::SubSelect {
            select: Box::new(inner_select),
            alias: Some(alias.to_owned()),
        },
        columns: selection_aggregate,
        predicate: ConcretePredicate::True,
        order_by: None,
        offset: None,
        limit: None,
        group_by: None,
        top_level_selection: selection_level == SelectionLevel::TopLevel,
    }
}

/// Compute the join and a suitable predicate for the given base table and predicate.
pub(super) fn join_info(
    base_table_id: TableId,
    predicate: &AbstractPredicate,
    predicate_column_paths: Vec<PhysicalColumnPath>,
    order_by_column_paths: Vec<PhysicalColumnPath>,
    additional_predicate: Option<ConcretePredicate>,
    transformer: &Postgres,
    database: &Database,
) -> (Table, ConcretePredicate) {
    let columns_paths: Vec<_> = predicate_column_paths
        .into_iter()
        .chain(order_by_column_paths.into_iter())
        .collect();

    let join = join_util::compute_join(base_table_id, &columns_paths);
    let predicate = transformer.to_predicate(predicate, true, database);

    let predicate = ConcretePredicate::and(
        predicate,
        additional_predicate.unwrap_or(ConcretePredicate::True),
    );

    (join, predicate)
}
