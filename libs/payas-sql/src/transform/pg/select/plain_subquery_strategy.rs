use crate::{
    sql::{predicate::ConcretePredicate, select::Select},
    transform::{join_util, transformer::PredicateTransformer},
};

use super::{
    selection_context::SelectionContext,
    selection_strategy::{compute_inner_select, nest_subselect, SelectionStrategy},
};

pub(crate) struct PlainSubqueryStrategy {}

/// Strategy that uses a single subquery
///
/// Suitable for many-to-one predicate with order-by, limit or offset
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
impl SelectionStrategy for PlainSubqueryStrategy {
    fn id(&self) -> &'static str {
        "PlainSubqueryStrategy"
    }

    fn suitable(&self, selection_context: &SelectionContext) -> bool {
        !selection_context.has_a_one_to_many_predicate
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

        let inner_select = compute_inner_select(
            join,
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
