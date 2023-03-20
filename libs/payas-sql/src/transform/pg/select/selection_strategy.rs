use crate::{
    sql::{predicate::ConcretePredicate, select::Select, table::Table},
    transform::{
        join_util,
        transformer::{OrderByTransformer, PredicateTransformer},
        SelectionLevel,
    },
    Column,
};

use super::selection_context::SelectionContext;

pub(crate) trait SelectionStrategy {
    fn id(&self) -> &'static str;

    fn suitable(&self, selection_context: &SelectionContext) -> bool;

    fn to_select<'a>(&self, selection_context: SelectionContext<'_, 'a>) -> Select<'a>;
}

pub(crate) struct SelectionStrategyChain<'s> {
    strategies: Vec<&'s dyn SelectionStrategy>,
}

impl<'s> SelectionStrategyChain<'s> {
    pub fn new(strategies: Vec<&'s dyn SelectionStrategy>) -> Self {
        Self { strategies }
    }

    pub fn to_select<'a>(&self, selection_context: SelectionContext<'_, 'a>) -> Option<Select<'a>> {
        let strategy = self
            .strategies
            .iter()
            .find(|s| s.suitable(&selection_context))?;

        println!("Using selection strategy: {}", strategy.id());

        Some(strategy.to_select(selection_context))
    }
}

/// Strategy for root-level or many-to-one predicate without order-by, limit or offset
///
/// Pre-conditions:
/// - Order by is none
/// - Limit and offsets are none
/// - Predicate meets one of the following conditions:
///     - No predicate (such as `concerts { ... }`)
///     - Predicate referring to columns of the root table (such as `concerts(where: {id: {gt: 10}})`)
///     - Predicate referring to columns of a table navigable though many-to-one relationships such as:
///         - `concerts(where: {venue: {id: {gt: 10}}})`
///         - `concerts(where: {venue: {city: {name: {eq: "San Francisco"}}}})`)
///
/// If the predicate refers to a single table, we can build the aggregate directly from the table's
/// data to produce a statement like:
///
/// ```sql
/// SELECT COALESCE(...)::text FROM "concerts" WHERE "concerts"."id" > $1
/// ```
///
/// If the predicate refers to multiple tables connected by many-to-one relationships, we use a join of the tables.
/// For example, consider the following query:
///
/// ```graphql
/// {
///   concerts(where: {venue: {id: {gt: 10}}}) {
///     ...
///   }
/// }
/// ```
/// Here, we join the root table with the one that the predicate refers and produce a statement
/// like:
///
/// ```sql
/// SELECT COALESCE(...)::text FROM "concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id" WHERE "venues"."id" < $1
/// ```
/// We can do the same for any level of predicates as long as relationships are of many-to-one kind.
///
/// These single-table and multi-many-to-one work in the same conceptual way, since with a single
/// table, the "join" becomes just the table itself.
///
/// :::caution
/// The many-to-one is a critical constraint while forming such queries. Otherwise, if a one-to-many
/// relationship is involved, the join will return multiple rows for each "many" side, and we will
/// end up with duplicate entries in the result.
/// :::
pub(crate) struct ManyToOnePredicateWithoutOrderWithoutLimitOffset {}

impl SelectionStrategy for ManyToOnePredicateWithoutOrderWithoutLimitOffset {
    fn id(&self) -> &'static str {
        "ManyToOnePredicateWithoutOrderWithoutLimitOffset"
    }

    fn suitable(&self, selection_context: &SelectionContext) -> bool {
        let no_non_predicate_clauses = selection_context.abstract_select.order_by.is_none()
            && selection_context.abstract_select.offset.is_none()
            && selection_context.abstract_select.limit.is_none();

        let allow_one_to_many = !selection_context.is_return_value_agg;

        (no_non_predicate_clauses && !selection_context.has_a_one_to_many_clause)
            || allow_one_to_many
    }

    fn to_select<'a>(&self, selection_context: SelectionContext<'_, 'a>) -> Select<'a> {
        let SelectionContext {
            abstract_select,
            additional_predicate,
            predicate_column_paths,
            order_by_column_paths,
            selection_level,
            transformer,
            ..
        } = selection_context;

        let predicate = ConcretePredicate::and(
            transformer.to_join_predicate(&abstract_select.predicate),
            additional_predicate.unwrap_or(ConcretePredicate::True),
        );

        let column_paths = predicate_column_paths
            .into_iter()
            .chain(order_by_column_paths.into_iter())
            .collect::<Vec<_>>();

        let join = join_util::compute_join(abstract_select.table, &column_paths);

        let selection_aggregate = abstract_select.selection.selection_aggregate(transformer);

        Select {
            table: join,
            columns: selection_aggregate,
            predicate,
            order_by: abstract_select
                .order_by
                .as_ref()
                .map(|ob| transformer.to_order_by(ob)),
            offset: abstract_select.offset.clone(),
            limit: abstract_select.limit.clone(),
            group_by: None,
            top_level_selection: selection_level == SelectionLevel::TopLevel,
        }
    }
}

pub(crate) struct ManyToOnePredicateWithOrderLimitOffset {}

/// Strategy for root-level or many-to-one predicate with order-by, limit or offset
///
/// Pre-conditions:
/// - Predicate meets one of the following conditions:
///     - No predicate (such as `concerts { ... }`)
///     - Predicate referring to columns of the root table (such as `concerts(where: {id: {gt: 10}})`)
///     - Predicate referring to columns of a table navigable though many-to-one relationships such as:
///         - `concerts(where: {venue: {id: {gt: 10}}})`
///         - `concerts(where: {venue: {city: {name: {eq: "San Francisco"}}}})`)
///
/// No restrictions on the order-by, limit or offset (there is an implicit restriction that the
/// order by must not specify a one-to-many relationship, since it will be ill-formed, anyway).
///
/// When the selection has an order by, limit or offset, we use a subselect in which that we can use
/// those clauses (note that compared to the earlier case, we wouldn't be able to simply add an
/// "order by", since that doesn't make sense when the return value is an aggregate (like a
/// `json_agg`)).
///
/// Since we can provide an order by, limit, or offset cause only to a bulk query, we need
/// not worry about the single-row case.
///
/// Here we produce a statement like:
///
/// ```sql
/// SELECT COALESCE(...)::text FROM (
///     SELECT "concerts".* FROM "concerts" WHERE "concerts"."id" > $1 LIMIT $2 OFFSET $3
/// ) AS "concerts"
/// ```
///
/// It is the subselect's job to apply the order by, limit and offset and return all the columns of
/// the table. (A possible optimization would be to only return the columns that are needed by the
/// aggregate, but we will need to, of course, benchmark this).
impl SelectionStrategy for ManyToOnePredicateWithOrderLimitOffset {
    fn id(&self) -> &'static str {
        "ManyToOnePredicateWithOrderLimitOffset"
    }

    fn suitable(&self, selection_context: &SelectionContext) -> bool {
        !selection_context.has_a_one_to_many_clause
    }

    fn to_select<'a>(&self, selection_context: SelectionContext<'_, 'a>) -> Select<'a> {
        let SelectionContext {
            abstract_select,
            additional_predicate,
            selection_level,
            predicate_column_paths,
            order_by_column_paths,
            transformer,
            ..
        } = selection_context;

        let columns_paths: Vec<_> = predicate_column_paths
            .into_iter()
            .chain(order_by_column_paths.into_iter())
            .collect();

        let join = join_util::compute_join(abstract_select.table, &columns_paths);
        let predicate = transformer.to_join_predicate(&abstract_select.predicate);

        let predicate = ConcretePredicate::and(
            predicate,
            additional_predicate.unwrap_or(ConcretePredicate::True),
        );

        let inner_select = Select {
            table: join,
            columns: vec![Column::Star(Some(abstract_select.table.name.clone()))],
            predicate,
            order_by: abstract_select
                .order_by
                .as_ref()
                .map(|ob| transformer.to_order_by(ob)),
            offset: abstract_select.offset.clone(),
            limit: abstract_select.limit.clone(),
            group_by: None,
            top_level_selection: false,
        };

        let selection_aggregate = abstract_select.selection.selection_aggregate(transformer);

        Select {
            table: Table::SubSelect {
                select: Box::new(inner_select),
                alias: Some(abstract_select.table.name.clone()),
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
}

pub struct OneToManyPredicateWithRootLevelOrderByWithLimitOffset {}

/// Strategy for one-to-many predicates with an optional order by for root-level fields, limit/offset may be present
///
/// Pre-conditions:
/// - Predicate unrestricted (but an overkill for the cases without one-to-many predicates)
/// - Order by restricted to the columns of the root table
/// - Limit and offset may be present
///
/// An example of this is:
/// ```graphql
/// {
///    venues(where: {concerts: {id: {lt: 100}}}) {
///      ...
///    }
/// }
/// ```
/// Here, we need to use a subselect to filter the rows of the table. We will produce a statement:
/// ```sql
/// SELECT COALESCE(...)::text FROM (
///     SELECT "venues".* FROM "venues" WHERE "venues"."id" IN (
///         SELECT "concerts"."venue_id" FROM "concerts" WHERE "concerts"."id" < $1
///     )
/// ) AS "venues"
/// ```
///
/// If we had used the same implementation as in case 1, we would have gotten multiple identical
/// rows for each venue (as many rows as there are concerts for that venue). By using a subselect,
/// we pick up only one row for each matching concert (we could have used `GROUP BY` or `DISTINCT`
/// for the nested query to reduce items for the `IN` predicate, but we should benchmark before
/// adding either of those).
///
/// A variation of this case is when an order by is also provided, but the order by refers to
/// root-level fields.
///
/// ```graphql
/// {
///    venues(where: {concerts: {id: {lt: 100}}}, orderBy: {name: desc}, limit: 10, offset: 20) {
///      ...
///    }
/// }
/// ```
///
/// In this case, we use essentially the same subselect, but we add an order by
/// clause to it. We will produce a statement like:
/// ```sql
/// SELECT COALESCE(...)::text FROM (
///     SELECT "venues".* FROM "venues" WHERE "venues"."id" IN (
///         SELECT "concerts"."venue_id" FROM "concerts" WHERE "concerts"."id" < $1
///     ) ORDER BY "venues"."name" DESC) AS "venues"
/// ```
///
/// We apply the order by clause to the subselect that picks up the data and not with the `IN`
/// subselect. This is because the `IN` subselect is only used to filter the rows of the table, not
/// to order them (and even if we did supply the order by, `IN` doesn't guarantee that the order
/// will be preserved).
///
/// Note that order by a column in the "many" table is not supported (such as "order venues by its
/// concerts"). Those are ill-defined operations, since there can be multiple rows for each "one"
/// row.
///
impl SelectionStrategy for OneToManyPredicateWithRootLevelOrderByWithLimitOffset {
    fn id(&self) -> &'static str {
        "OneToManyPredicateWithRootLevelOrderByWithLimitOffset"
    }

    fn suitable(&self, selection_context: &SelectionContext) -> bool {
        selection_context.order_by_column_paths.is_empty()
            || selection_context
                .order_by_column_paths
                .iter()
                .all(|path| path.len() <= 1)
    }

    fn to_select<'a>(&self, selection_context: SelectionContext<'_, 'a>) -> Select<'a> {
        let SelectionContext {
            abstract_select,
            additional_predicate,
            selection_level,
            transformer,
            ..
        } = selection_context;

        let predicate = transformer.to_subselect_predicate(&abstract_select.predicate);
        let table = Table::Physical(abstract_select.table);
        let predicate = ConcretePredicate::and(
            predicate,
            additional_predicate.unwrap_or(ConcretePredicate::True),
        );

        // Inner select gives the data matching the predicate, order by, offset, limit
        let inner_select = Select {
            table,
            columns: vec![Column::Star(Some(abstract_select.table.name.clone()))],
            predicate,
            order_by: abstract_select
                .order_by
                .as_ref()
                .map(|ob| transformer.to_order_by(ob)),
            offset: abstract_select.offset.clone(),
            limit: abstract_select.limit.clone(),
            group_by: None,
            top_level_selection: false,
        };

        let selection_aggregate = abstract_select.selection.selection_aggregate(transformer);

        // We then use the inner select to build the final select
        Select {
            table: Table::SubSelect {
                select: Box::new(inner_select),
                alias: Some(abstract_select.table.name.clone()),
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
}
pub struct Unconditional {}

impl SelectionStrategy for Unconditional {
    fn id(&self) -> &'static str {
        "Unconditional"
    }

    fn suitable(&self, _selection_context: &SelectionContext) -> bool {
        true
    }

    fn to_select<'a>(&self, selection_context: SelectionContext<'_, 'a>) -> Select<'a> {
        let SelectionContext {
            abstract_select,
            additional_predicate,
            selection_level,
            predicate_column_paths,
            order_by_column_paths,
            transformer,
            ..
        } = selection_context;
        let columns_paths = predicate_column_paths
            .into_iter()
            .chain(order_by_column_paths.into_iter())
            .collect::<Vec<_>>();

        let predicate = transformer.to_join_predicate(&abstract_select.predicate);
        let join = join_util::compute_join(abstract_select.table, &columns_paths);

        let inner_select = Select {
            table: join,
            columns: vec![Column::Physical(
                abstract_select.table.get_pk_physical_column().unwrap(),
            )],
            predicate: ConcretePredicate::In(
                Column::Physical(abstract_select.table.get_pk_physical_column().unwrap()),
                Column::SubSelect(Box::new(Select {
                    table: Table::Physical(abstract_select.table),
                    columns: vec![Column::Physical(
                        abstract_select.table.get_pk_physical_column().unwrap(),
                    )],
                    predicate,
                    order_by: abstract_select
                        .order_by
                        .as_ref()
                        .map(|ob| transformer.to_order_by(ob)),
                    offset: abstract_select.offset.clone(),
                    limit: abstract_select.limit.clone(),
                    group_by: None,
                    top_level_selection: false,
                })),
            ),
            order_by: None,
            offset: None,
            limit: None,
            group_by: None,
            top_level_selection: false,
        };

        let predicate = ConcretePredicate::In(
            Column::Physical(abstract_select.table.get_pk_physical_column().unwrap()),
            Column::SubSelect(Box::new(inner_select)),
        );
        let table = Table::Physical(abstract_select.table);

        let predicate = ConcretePredicate::and(
            predicate,
            additional_predicate.unwrap_or(ConcretePredicate::True),
        );

        // Inner select gives the data matching the predicate, order by, offset, limit
        let inner_select = Select {
            table,
            columns: vec![Column::Star(Some(abstract_select.table.name.clone()))],
            predicate,
            order_by: abstract_select
                .order_by
                .as_ref()
                .map(|ob| transformer.to_order_by(ob)),
            offset: abstract_select.offset.clone(),
            limit: abstract_select.limit.clone(),
            group_by: None,
            top_level_selection: false,
        };

        let selection_aggregate = abstract_select.selection.selection_aggregate(transformer);

        // We then use the inner select to build the final select
        Select {
            table: Table::SubSelect {
                select: Box::new(inner_select),
                alias: Some(abstract_select.table.name.clone()),
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
}
