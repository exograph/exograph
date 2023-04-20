// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    sql::predicate::ConcretePredicate,
    transform::pg::{Postgres, SelectionLevel},
    AbstractSelect, ColumnPath, ColumnPathLink,
};

/// A context for the selection transformation to avoid repeating the same work
/// by each strategy.
pub(crate) struct SelectionContext<'c, 'a> {
    pub abstract_select: &'c AbstractSelect<'a>,
    pub has_a_one_to_many_predicate: bool,
    pub predicate_column_paths: Vec<Vec<ColumnPathLink<'a>>>,
    pub order_by_column_paths: Vec<Vec<ColumnPathLink<'a>>>,
    pub additional_predicate: Option<ConcretePredicate<'a>>,
    pub selection_level: SelectionLevel,
    pub allow_duplicate_rows: bool,
    pub transformer: &'c Postgres,
}

impl<'c, 'a> SelectionContext<'c, 'a> {
    pub fn new(
        abstract_select: &'c AbstractSelect<'a>,
        additional_predicate: Option<ConcretePredicate<'a>>,
        selection_level: SelectionLevel,
        allow_duplicate_rows: bool,
        transformer: &'c Postgres,
    ) -> Self {
        fn column_path_owned<'a>(
            column_paths: Vec<&ColumnPath<'a>>,
        ) -> Vec<Vec<ColumnPathLink<'a>>> {
            column_paths
                .into_iter()
                .filter_map(|path| match path {
                    ColumnPath::Physical(links) => Some(links.to_vec()),
                    _ => None,
                })
                .collect()
        }

        let predicate_column_paths: Vec<Vec<ColumnPathLink>> =
            column_path_owned(abstract_select.predicate.column_paths());

        let order_by_column_paths = abstract_select
            .order_by
            .as_ref()
            .map(|ob| column_path_owned(ob.column_paths()))
            .unwrap_or_else(Vec::new);

        // Sanity check that there are no one-to-many links in the order by clause
        // such a clause would be ill-formed
        order_by_column_paths
            .iter()
            .for_each(|path| path.iter().for_each(|link| assert!(!link.is_one_to_many())));

        let has_a_one_to_many_predicate = predicate_column_paths
            .iter()
            .any(|path| path.iter().any(|link| link.is_one_to_many()));

        Self {
            abstract_select,
            has_a_one_to_many_predicate,
            predicate_column_paths,
            order_by_column_paths,
            additional_predicate,
            selection_level,
            allow_duplicate_rows,
            transformer,
        }
    }
}
