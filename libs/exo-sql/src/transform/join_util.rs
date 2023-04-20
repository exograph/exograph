// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    asql::column_path::ColumnPathLink,
    sql::{column::Column, join::LeftJoin, predicate::ConcretePredicate, table::Table},
    transform::table_dependency::{DependencyLink, TableDependency},
    PhysicalTable,
};

/// Compute the join needed to access the leaf columns of a list of column paths. Will return a
/// `Table::Physical` if there are no dependencies to join otherwise a `Table::Join`.
pub fn compute_join<'a>(
    table: &'a PhysicalTable,
    paths_list: &[Vec<ColumnPathLink<'a>>],
) -> Table<'a> {
    /// Recursively build the join tree.
    fn from_dependency(dependency: TableDependency) -> Table {
        dependency.dependencies.into_iter().fold(
            Table::Physical(dependency.table),
            |acc, DependencyLink { link, dependency }| {
                let join_predicate = ConcretePredicate::Eq(
                    Column::Physical(link.self_column.0),
                    Column::Physical(link.linked_column.unwrap().0),
                );

                let join_table_query = from_dependency(dependency);

                Table::Join(LeftJoin::new(acc, join_table_query, join_predicate.into()))
            },
        )
    }

    let table_tree = TableDependency::from_column_path(paths_list).unwrap_or(TableDependency {
        table,
        dependencies: vec![],
    });
    from_dependency(table_tree)
}

#[cfg(test)]
mod tests {
    use crate::{
        asql::column_path::ColumnPathLink, sql::ExpressionBuilder, transform::test_util::TestSetup,
    };

    #[test]
    fn single_level_join() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 venues_table,
                 concerts_venue_id_column,
                 venues_id_column,
                 venues_name_column,
                 ..
             }| {
                // (concert.venue_id, venue.id) -> (venue.name, None)
                let concert_venue = vec![
                    ColumnPathLink {
                        self_column: (concerts_venue_id_column, concerts_table),
                        linked_column: Some((venues_id_column, venues_table)),
                    },
                    ColumnPathLink {
                        self_column: (venues_name_column, venues_table),
                        linked_column: None,
                    },
                ];

                let join = super::compute_join(concerts_table, &[concert_venue]);

                assert_binding!(
                    join.to_sql(),
                    r#""concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
                );
            },
        )
    }

    #[test]
    fn multi_level_join() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concert_artists_table,
                 artists_table,
                 addresses_table,
                 venues_table,

                 concerts_id_column,
                 concerts_venue_id_column,

                 concert_artists_concert_id_column,
                 concert_artists_artist_id_column,

                 artists_id_column,
                 artists_name_column,
                 artists_address_id_column,

                 addresses_id_column,
                 addresses_city_column,

                 venues_id_column,
                 venues_name_column,
                 ..
             }| {
                // (concert.id, concert_artists.concert_id) -> (concert_artists.artist_id, artists.id) -> (artists.name, None)
                let concert_ca_artist = vec![
                    ColumnPathLink {
                        self_column: (concerts_id_column, concerts_table),
                        linked_column: Some((
                            concert_artists_concert_id_column,
                            concert_artists_table,
                        )),
                    },
                    ColumnPathLink {
                        self_column: (concert_artists_artist_id_column, concert_artists_table),
                        linked_column: Some((artists_id_column, artists_table)),
                    },
                    ColumnPathLink {
                        self_column: (artists_name_column, artists_table),
                        linked_column: None,
                    },
                ];

                // (concert.id, concert_artists.concert_id) -> (concert_artists.artist_id, artists.id) -> (artists.address_id, address.id) -> (address.city, None)
                let concert_ca_artist_address = vec![
                    ColumnPathLink {
                        self_column: (concerts_id_column, concerts_table),
                        linked_column: Some((
                            concert_artists_concert_id_column,
                            concert_artists_table,
                        )),
                    },
                    ColumnPathLink {
                        self_column: (concert_artists_artist_id_column, concert_artists_table),
                        linked_column: Some((artists_id_column, artists_table)),
                    },
                    ColumnPathLink {
                        self_column: (artists_address_id_column, artists_table),
                        linked_column: Some((addresses_id_column, addresses_table)),
                    },
                    ColumnPathLink {
                        self_column: (addresses_city_column, addresses_table),
                        linked_column: None,
                    },
                ];

                // (concert.venue_id, venue.id) -> (venue.name, None)
                let concert_venue = vec![
                    ColumnPathLink {
                        self_column: (concerts_venue_id_column, concerts_table),
                        linked_column: Some((venues_id_column, venues_table)),
                    },
                    ColumnPathLink {
                        self_column: (venues_name_column, venues_table),
                        linked_column: None,
                    },
                ];

                let join = super::compute_join(
                    concerts_table,
                    &[concert_ca_artist, concert_ca_artist_address, concert_venue],
                );

                assert_binding!(
                    join.to_sql(),
                    r#""concerts" LEFT JOIN "concert_artists" LEFT JOIN "artists" LEFT JOIN "addresses" ON "artists"."address_id" = "addresses"."id" ON "concert_artists"."artist_id" = "artists"."id" ON "concerts"."id" = "concert_artists"."concert_id" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
                );
            },
        )
    }
}
