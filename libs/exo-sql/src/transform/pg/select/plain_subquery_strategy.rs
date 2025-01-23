// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{sql::select::Select, Database};

use super::{
    selection_context::SelectionContext,
    selection_strategy::{compute_inner_select, join_info, nest_subselect, SelectionStrategy},
};

/// Strategy that uses a single subquery
///
/// Suitable for many-to-one predicates with order-by, limit or offset
///
/// Pre-conditions (for aggregate output):
/// - Predicates that are only of many-to-one kind. Specifically, meeting one of the following conditions:
///     - No predicate (such as `concerts { ... }`)
///     - Predicate referring to columns of the root table (such as `concerts(where: {id: {gt: 10}})`)
///     - Predicate referring to columns of a table navigable though many-to-one relationships such as:
///         - `concerts(where: {venue: {id: {gt: 10}}})`
///         - `concerts(where: {venue: {city: {name: {eq: "San Francisco"}}}})`)
/// - No restrictions on the order-by, limit or offset (there is an implicit restriction that the
///   order by must not specify a one-to-many relationship, since it will be ill-formed, anyway and
///   is enforced in [`SelectionContext`]).
///
/// Pre-conditions (for non-aggregate output):
/// - Predicate along with order by refers to only many-to-one relationships (such as `concerts(orderBy: {venue: {name: ASC}})`)
///   OR
/// - Explicit specification indicated that duplicate rows are allowed
///
/// We use a subselect in which we can use the predicates, order-by, limit, and offset (note
/// that compared to the [`PlainJoinStrategy`] case, we wouldn't be able to simply add an "order
/// by", since that doesn't make sense when the return value is an aggregate (like a `json_agg`)).
///
/// Here we produce a statement like:
///
/// ```sql
/// SELECT COALESCE(...)::text FROM (
///     SELECT "concerts".* FROM "concerts" WHERE "concerts"."id" > $1 ORDER BY "concerts"."title" LIMIT $2 OFFSET $3
/// ) AS "concerts"
/// ```
///
/// or (if the predicate or order by refers to multiple tables):
///
/// /// ```sql
/// SELECT COALESCE(...)::text FROM (
///     SELECT "concerts".* FROM "concerts" LEFT JOIN "venues" on "concerts"."venue_id" = "venue"."id"
///         WHERE "concerts"."id" > $1 ORDER BY "venues"."name" LIMIT $2 OFFSET $3
/// ) AS "concerts"
///
/// It is the subselect's job to apply the order by, limit and offset and return all the columns of
/// the table. (TODO: A possible optimization would be to only return the columns that are needed by
/// the aggregate, but we will need to, of course, benchmark this).
pub(crate) struct PlainSubqueryStrategy {}

impl SelectionStrategy for PlainSubqueryStrategy {
    fn id(&self) -> &'static str {
        "PlainSubqueryStrategy"
    }

    fn suitable(&self, selection_context: &SelectionContext) -> bool {
        !selection_context.has_a_one_to_many_predicate || selection_context.allow_duplicate_rows
    }

    fn to_select(&self, selection_context: SelectionContext<'_>, database: &Database) -> Select {
        let SelectionContext {
            abstract_select,
            selection_level,
            predicate_column_paths,
            order_by_column_paths,
            transformer,
            ..
        } = selection_context;

        let (join, predicate) = join_info(
            abstract_select.table_id,
            &abstract_select.predicate,
            predicate_column_paths,
            order_by_column_paths,
            selection_level,
            transformer,
            database,
        );

        let inner_select = compute_inner_select(
            join,
            abstract_select.table_id,
            predicate,
            &abstract_select.order_by,
            &abstract_select.limit,
            &abstract_select.offset,
            selection_level,
            transformer,
            database,
        );

        let table = &database.get_table(abstract_select.table_id);
        let alias_info = (table.name.synthetic_name(), table.name.clone());

        nest_subselect(
            inner_select,
            abstract_select.selection,
            selection_level,
            alias_info,
            transformer,
            database,
        )
    }
}
