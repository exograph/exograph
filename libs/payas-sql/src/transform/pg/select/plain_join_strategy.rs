use crate::{
    sql::{predicate::ConcretePredicate, select::Select},
    transform::{
        join_util,
        transformer::{OrderByTransformer, PredicateTransformer},
        SelectionLevel,
    },
    Selection,
};

use super::{selection_context::SelectionContext, selection_strategy::SelectionStrategy};

/// Strategy that uses a plain join of tables invoked in clauses
///
/// Suitable for:
/// - For aggregate output: Many-to-one predicate without order-by, limit or offset
/// - For non-aggregate output: Many-to-one predicate or order-by; or duplicate rows can be tolerated
///
/// Pre-conditions (for aggregate output):
/// - Order by is none
/// - Limit and offsets are none
/// - Predicate meets one of the following conditions:
///     - No predicate (such as `concerts { ... }`)
///     - Predicate referring to columns of the root table (such as `concerts(where: {id: {gt: 10}})`)
///     - Predicate referring to columns of a table navigable though many-to-one relationships such as:
///         - `concerts(where: {venue: {id: {gt: 10}}})`
///         - `concerts(where: {venue: {city: {name: {eq: "San Francisco"}}}})`)
/// Pre-conditions (for non-aggregate output):
/// - Predicate along with order by refers to only many-to-one relationships (such as `concerts(orderBy: {venue: {name: asc}})`)
/// - Duplicate rows can be tolerated (such as as a subquery to form a predicate for a delete mutation)
///
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
pub(crate) struct PlainJoinStrategy {}

impl SelectionStrategy for PlainJoinStrategy {
    fn id(&self) -> &'static str {
        "PlainJoinStrategy"
    }

    fn suitable(&self, selection_context: &SelectionContext) -> bool {
        if matches!(
            selection_context.abstract_select.selection,
            Selection::Json(..)
        ) {
            // The expected output is a JSON object, so can't allow any non-predicate clauses
            // and we can't allow any one-to-many relationships (they will cause duplicate rows)
            let no_non_predicate_clauses = selection_context.abstract_select.order_by.is_none()
                && selection_context.abstract_select.offset.is_none()
                && selection_context.abstract_select.limit.is_none();

            no_non_predicate_clauses && !selection_context.has_a_one_to_many_predicate
        } else {
            // The expected output is non-aggregate, so if duplicated rows are allowed, this
            // is a suitable strategy (typically useful as a column in `IN` predicate such as that used by delete)
            !selection_context.has_a_one_to_many_predicate || selection_context.allow_duplicate_rows
        }
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
