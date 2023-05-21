// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::BTreeMap;

use crate::{asql::column_path::PhysicalColumnPathLink, TableId};

#[derive(Debug)]
pub struct TableDependency {
    /// The base table being joined. In the example below (in impl TableDependency), "concerts"
    pub table_id: TableId,
    /// The tables being joined. In the example below, ("venue1_id", "venues") and ("venue2_id", "venues")
    pub dependencies: Vec<DependencyLink>,
}

#[derive(Debug)]
pub struct DependencyLink {
    pub link: PhysicalColumnPathLink,
    pub dependency: TableDependency,
}

impl TableDependency {
    /// Compute TableDependency from a list of column paths
    /// If the following path is given:
    /// ```no_rust
    /// [
    ///     (concert.id, concert_artists.concert_id) -> (concert_artists.artist_id, artists.id) -> (artists.name, None)
    ///     (concert.id, concert_artists.concert_id) -> (concert_artists.artist_id, artists.id) -> (artists.address_id, address.id) -> (address.city, None)
    ///     (concert.venue_id, venue.id) -> (venue.name, None)
    /// ]
    /// ```
    /// then the result will be the join needed to access the leaf columns:
    /// ```no_rust
    /// TableDependency {
    ///    table: concerts,
    ///    dependencies: [
    ///       ((concert.id, concert_artists.concert_id), TableDependency {
    ///          table: concert_artists,
    ///          dependencies: [
    ///             ((concert_artists.artist_id, artists.id), TableDependency {
    ///                table: artists,
    ///                dependencies: [
    ///                   ((artists.address_id, address.id), TableDependency {
    ///                      table: address,
    ///                      dependencies: []
    ///                   }),
    ///                ]
    ///             }),
    ///       ((concert.venue_id, venue.id), TableDependency {
    ///            table: venue,
    ///            dependencies: []
    ///       }),
    ///    ]
    /// }
    /// ```
    pub fn from_column_path(paths_list: &[Vec<PhysicalColumnPathLink>]) -> Option<Self> {
        let table_id = paths_list.get(0)?.get(0)?.self_column_id.table_id;

        assert!(
            paths_list
                .iter()
                .all(|path| path.get(0).unwrap().self_column_id.table_id == table_id),
            "All paths must start from the same table"
        );

        // Use `BTreeMap` to get a stable ordering of the dependencies
        // (mostly for testing purpose, but also to get predictable results)
        //
        // Group by the `ColumnIdPathLink` to paths that start with it.
        // Later the key (`ColumnIdPathLink`) and values (`Vec<ColumnIdPathLink>`) will
        // be used to create `DependencyLink`s.
        let grouped = paths_list.iter().fold(
            BTreeMap::<PhysicalColumnPathLink, Vec<Vec<PhysicalColumnPathLink>>>::new(),
            |mut acc, paths| match &paths[..] {
                [head, tail @ ..] => {
                    if head.linked_column_id.is_some() {
                        acc.entry(head.clone()).or_default().push(tail.to_vec());
                    }
                    acc
                }
                _ => {
                    panic!("Invalid paths list. Must have at least one path");
                }
            },
        );

        let dependencies = grouped
            .into_iter()
            .map(|(link, paths)| {
                let dependency = Self::from_column_path(&paths).unwrap();
                DependencyLink { link, dependency }
            })
            .collect();

        Some(Self {
            table_id,
            dependencies,
        })
    }
}
