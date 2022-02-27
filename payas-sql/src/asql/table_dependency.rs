use std::collections::BTreeMap;

use crate::sql::PhysicalTable;

use super::column_path::ColumnPathLink;

#[derive(Debug)]
pub struct TableDependency<'a> {
    /// The base table being joined. In the example below, "concerts"
    pub table: &'a PhysicalTable,
    /// The tables being joined. In the example below, ("venue1_id", "venues") and ("venue2_id", "venues")
    pub dependencies: Vec<DependencyLink<'a>>,
}

#[derive(Debug)]
pub struct DependencyLink<'a> {
    pub link: ColumnPathLink<'a>,
    pub dependency: TableDependency<'a>,
}

impl<'a> TableDependency<'a> {
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
    /// TableJoin {
    ///    table: concerts,
    ///    dependencies: [
    ///       ((concert.id, concert_artists.concert_id), TableJoin {
    ///          table: concert_artists,
    ///          dependencies: [
    ///             ((concert_artists.artist_id, artists.id), TableJoin {
    ///                table: artists,
    ///                dependencies: [
    ///                   ((artists.address_id, address.id), TableJoin {
    ///                      table: address,
    ///                      dependencies: []
    ///                   }),
    ///                ]
    ///             }),
    ///       ((concert.venue_id, venue.id), TableJoin {
    ///            table: venue,
    ///            dependencies: []
    ///       }),
    ///    ]
    /// }
    /// ```
    pub fn from_column_path(paths_list: Vec<Vec<ColumnPathLink<'a>>>) -> Option<Self> {
        let table = paths_list.get(0)?.get(0)?.self_column.1;

        assert!(
            paths_list
                .iter()
                .all(|path| path.get(0).unwrap().self_column.1 == table),
            "All paths must start from the same table"
        );

        // Use `BTreeMap` to get a stable ordering of the dependencies
        // (mostly for testing purpose, but also to get predictable results)
        //
        // Group by the `ColumnPathLink` to paths that start with it.
        // Later the key (`ColumnPathLink`) and values (`Vec<ColumnPathLink>`) will
        // be used to create `DependencyLink`s.
        let grouped = paths_list.into_iter().fold(
            BTreeMap::<ColumnPathLink, Vec<Vec<ColumnPathLink<'a>>>>::new(),
            |mut acc, paths| match &paths[..] {
                [head, tail @ ..] => {
                    if head.linked_column.is_some() {
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
                let dependency = Self::from_column_path(paths).unwrap();
                DependencyLink { link, dependency }
            })
            .collect();

        Some(Self {
            table,
            dependencies,
        })
    }
}
