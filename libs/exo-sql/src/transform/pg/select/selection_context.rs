// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    AbstractSelect, ColumnPath, Database, PhysicalColumnPath,
    transform::pg::{Postgres, selection_level::SelectionLevel},
};

/// A context for the selection transformation to avoid repeating the same work
/// by each strategy.
pub(crate) struct SelectionContext<'c> {
    pub abstract_select: AbstractSelect,
    pub has_a_one_to_many_predicate: bool,
    pub predicate_column_paths: Vec<PhysicalColumnPath>,
    pub order_by_column_paths: Vec<PhysicalColumnPath>,
    pub selection_level: &'c SelectionLevel,
    pub allow_duplicate_rows: bool,
    pub transformer: &'c Postgres,
}

impl<'c> SelectionContext<'c> {
    pub fn new(
        database: &Database,
        abstract_select: AbstractSelect,
        selection_level: &'c SelectionLevel,
        allow_duplicate_rows: bool,
        transformer: &'c Postgres,
    ) -> Self {
        let predicate_column_paths: Vec<_> = abstract_select
            .predicate
            .column_paths()
            .iter()
            .flat_map(|p| match p {
                ColumnPath::Physical(links) => Some(links.clone()),
                _ => None,
            })
            .collect();

        let order_by_column_paths: Vec<_> = abstract_select
            .order_by
            .as_ref()
            .map(|ob| {
                ob.column_paths()
                    .iter()
                    .map(|p| <&PhysicalColumnPath>::clone(p).clone())
                    .collect()
            })
            .unwrap_or_default();

        // Sanity check that there are no one-to-many links in the order by clause
        // such a clause would be ill-formed
        order_by_column_paths
            .iter()
            .for_each(|path| assert!(!path.has_one_to_many(database)));

        let has_a_one_to_many_predicate = predicate_column_paths
            .iter()
            .any(|path| path.has_one_to_many(database));

        Self {
            abstract_select,
            has_a_one_to_many_predicate,
            predicate_column_paths,
            order_by_column_paths,
            selection_level,
            allow_duplicate_rows,
            transformer,
        }
    }
}
