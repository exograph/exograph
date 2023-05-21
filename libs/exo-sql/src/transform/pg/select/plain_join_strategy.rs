// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    sql::select::Select,
    transform::{pg::SelectionLevel, transformer::OrderByTransformer},
    Database, Selection,
};

use super::{
    selection_context::SelectionContext,
    selection_strategy::{join_info, SelectionStrategy},
};

/// Strategy that uses a plain join of tables involved in clauses
///
/// Suitable for:
/// - For aggregate output: Only many-to-one predicates without order-by, limit or offset.
/// - For non-aggregate output: Only many-to-one predicates or order-by; or where duplicate rows can
///   be tolerated (typically for a subquery)
///
/// Pre-conditions (for aggregate output):
/// - Order by is none
/// - Limit and offsets are none
/// - Predicates that are only of many-to-one kind. Specifically, meeting one of the following conditions:
///     - No predicate (such as `concerts { ... }`)
///     - Predicate referring to columns of the root table (such as `concerts(where: {id: {gt: 10}})`)
///     - Predicate referring to columns of a table navigable though many-to-one relationships such as:
///         - `concerts(where: {venue: {id: {gt: 10}}})`
///         - `concerts(where: {venue: {city: {name: {eq: "San Francisco"}}}})`)
///
/// Pre-conditions (for non-aggregate output):
/// - Predicate along with order-by refers to only many-to-one relationships (such as `concerts(orderBy: {venue: {name: ASC}})`)
///   -- or --
/// - Explicit specification indicating that duplicate rows are allowed
///
/// If the predicate refers to a single table, we build the aggregate directly from the table's data
/// to produce a statement like:
///
/// ```sql
/// SELECT COALESCE(...)::text FROM "concerts" WHERE "concerts"."id" > $1
/// ```
///
/// If the predicate refers to multiple tables connected by many-to-one relationships, we use a join
/// of the tables. For example, consider the following query:
///
/// ```graphql
/// {
///   concerts(where: {venue: {id: {gt: 10}}}) {
///     ...
///   }
/// }
/// ```
/// Here, we join the root table with the one that the predicate refers to produce a statement like:
///
/// ```sql
/// SELECT COALESCE(...)::text FROM "concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id" WHERE "venues"."id" < $1
/// ```
/// We can do the same for any depth of predicates as long as the relationships are of many-to-one kind.
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

    fn to_select<'a>(
        &self,
        selection_context: SelectionContext<'_, 'a>,
        database: &'a Database,
    ) -> Select<'a> {
        let SelectionContext {
            abstract_select,
            additional_predicate,
            predicate_column_paths,
            order_by_column_paths,
            selection_level,
            transformer,
            ..
        } = selection_context;

        let (join, predicate) = join_info(
            abstract_select.table_id,
            &abstract_select.predicate,
            predicate_column_paths,
            order_by_column_paths,
            additional_predicate,
            transformer,
            database,
        );

        let selection_aggregate = abstract_select
            .selection
            .selection_aggregate(transformer, database);

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
