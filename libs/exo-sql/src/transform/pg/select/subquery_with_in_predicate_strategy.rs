use crate::{
    sql::{predicate::ConcretePredicate, select::Select},
    transform::{join_util, transformer::PredicateTransformer},
};

use super::{
    selection_context::SelectionContext,
    selection_strategy::{compute_inner_select, nest_subselect, SelectionStrategy},
};

/// Strategy that uses a subquery with an `IN` predicate to filter the rows of the table.
///
/// Suitable for any kind of relationship (with more suitability for one-to-many predicates) with an
/// optional order by, limit/offset may be present (this is a catch-all strategy that is used when
/// no other strategy is suitable).
///
/// Pre-conditions:
/// - None
///
/// An example of this is:
/// ```graphql
/// {
///    venues(where: {concerts: {id: {lt: 100}}}) {
///      ...
///    }
/// }
/// ```
/// Here, we use a subselect to filter the rows of the table to produce a statement like:
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
/// A variation of this case is when an order-by is also provided, but the order-by refers to
/// root-level fields.
///
/// ```graphql
/// {
///    venues(where: {concerts: {id: {lt: 100}}}, orderBy: {name: DESC}, limit: 10, offset: 20) {
///      ...
///    }
/// }
/// ```
///
/// In this case, we add an order-by clause to the above query to produce a statement like:
///
/// ```sql
/// SELECT COALESCE(...)::text FROM (
///     SELECT "venues".* FROM "venues" WHERE "venues"."id" IN (
///         SELECT "concerts"."venue_id" FROM "concerts" WHERE "concerts"."id" < $1
///     ) ORDER BY "venues"."name" DESC) AS "venues"
/// ```
///
/// We apply the order-by clause to the subselect that picks up the data and not with the `IN`
/// subselect. This is because the `IN` subselect is only used to filter the rows of the table, not
/// to order them (and even if we did supply the order by, `IN` doesn't guarantee that the order
/// will be preserved).
///
/// If an order-by refers to a field in a related table, we use the same strategy except we form a
/// join with the tables involved in the order-by. For example:
///
/// ```graphql
///  notifications(
///     where: {or: [{title: {ilike: '$search'}}, {concert: {or: [{title: {ilike: $search}}, {concertArtists: {artist: {name: {ilike: $search}}}}]}}]}
///     orderBy: {concert: {title: ASC}}
///     limit: 20
///     offset: 1o
///  ) {
///     id
///  }
/// ```
/// We will produce a statement like:
/// ```sql
/// SELECT COALESCE(json_agg(json_build_object('id', "notifications"."id")), '[]'::json)::text FROM (
///     SELECT "notifications".* FROM "notifications" LEFT JOIN "concerts" ON "notifications"."concert_id" = "concerts"."id"
///         WHERE "notifications"."id" IN (
///           SELECT "notifications"."id" FROM "notifications" LEFT JOIN "concerts" LEFT JOIN "concert_artists"
///             ON "concerts"."id" = "concert_artists"."concert_id" ON "notifications"."concert_id" = "concerts"."id" WHERE ... <predicate>
///         ) ORDER BY "concerts"."title" DESC LIMIT 20 OFFSET 10
///   )  AS "notifications";
/// ```
///
/// Here, instead of using "notifications", we use a join to involve "concerts", since the order by uses its column.
///
/// Note that order-by a column in the "many" table is not supported (such as "order venues by its
/// concerts"). Those are ill-defined operations, since there can be multiple rows for each "one"
/// row.
pub struct SubqueryWithInPredicateStrategy {}

impl SelectionStrategy for SubqueryWithInPredicateStrategy {
    fn id(&self) -> &'static str {
        "SubqueryWithInPredicateStrategy"
    }

    fn suitable(&self, _selection_context: &SelectionContext) -> bool {
        true
    }

    fn to_select<'a>(&self, selection_context: SelectionContext<'_, 'a>) -> Select<'a> {
        let SelectionContext {
            abstract_select,
            additional_predicate,
            order_by_column_paths,
            selection_level,
            transformer,
            ..
        } = selection_context;

        // Use only order by columns to form the join, since the predicate part is already taken
        // care by the `to_subselect_predicate` call above. The use of `order_by_column_paths` is
        // essential to be able to refer to columns in related field in the order by clause.
        let table = join_util::compute_join(abstract_select.table, &order_by_column_paths);

        // We don't use the the columns specified in the abstract predicate to form the join (we use
        // only order-by), so we let the predicate transformer know that it should not assume that
        // all tables are joined.
        let predicate = transformer.to_predicate(&abstract_select.predicate, false);
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
