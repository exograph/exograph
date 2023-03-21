use crate::{
    sql::{predicate::ConcretePredicate, select::Select, table::Table},
    transform::transformer::PredicateTransformer,
};

use super::{
    selection_context::SelectionContext,
    selection_strategy::{compute_inner_select, nest_subselect, SelectionStrategy},
};

pub struct SubqueryWithInPredicateStrategy {}

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
/// If we had used the same implementation as [`PlainJoinStrategy`], we would have gotten multiple
/// identical rows for each venue (as many rows as there are concerts for that venue). By using a
/// subselect, we pick up only one row for each matching concert (we could have used `GROUP BY` or
/// `DISTINCT` for the nested query to reduce items for the `IN` predicate, but we should benchmark
/// before adding either of those).
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
impl SelectionStrategy for SubqueryWithInPredicateStrategy {
    fn id(&self) -> &'static str {
        "SubqueryWithInPredicateStrategy"
    }

    fn suitable(&self, selection_context: &SelectionContext) -> bool {
        selection_context
            .order_by_column_paths
            .iter()
            .all(|path| path.len() == 1)
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

        let inner_select = compute_inner_select(
            table,
            abstract_select.table,
            predicate,
            &abstract_select.order_by,
            &abstract_select.limit,
            &abstract_select.offset,
            transformer,
        );

        nest_subselect(
            inner_select,
            &abstract_select.selection,
            selection_level,
            &abstract_select.table.name,
            transformer,
        )
    }
}
