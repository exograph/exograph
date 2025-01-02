// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    asql::column_path::{ColumnPathLink, RelationLink},
    sql::{
        column::Column, join::LeftJoin, predicate::ConcretePredicate, relation::RelationColumnPair,
        table::Table,
    },
    transform::table_dependency::{DependencyLink, TableDependency},
    Database, PhysicalColumnPath, TableId,
};

use super::pg::selection_level::SelectionLevel;

/// Compute the join needed to access the leaf columns of a list of column paths. Will return a
/// `Table::Physical` if there are no dependencies to join otherwise a `Table::Join`.
pub fn compute_join(
    table_id: TableId,
    paths_list: &[PhysicalColumnPath],
    selection_level: &SelectionLevel,
    database: &Database,
) -> Table {
    /// Recursively build the join tree.
    fn from_dependency(
        dependency: TableDependency,
        selection_level: &SelectionLevel,
        database: &Database,
        top_level: bool,
    ) -> Table {
        // We don't use the alias for the top level table in predicate, so match the behavior here
        let init_table = {
            Table::physical(
                dependency.table_id,
                if top_level {
                    None
                } else {
                    Some(selection_level.alias((dependency.table_id, None), database))
                },
            )
        };

        dependency.dependencies.into_iter().fold(
            init_table,
            |acc, DependencyLink { link, dependency }| {
                let (join_predicate, linked_table_alias) = match link {
                    ColumnPathLink::Relation(RelationLink {
                        column_pairs,
                        linked_table_alias,
                    }) => {
                        let RelationColumnPair {
                            self_column_id,
                            foreign_column_id: linked_column_id,
                        } = column_pairs[0];
                        let new_alias = selection_level
                            .alias((linked_column_id.table_id, linked_table_alias), database);

                        (
                            ConcretePredicate::Eq(
                                Column::physical(self_column_id, None),
                                Column::physical(linked_column_id, Some(new_alias.clone())),
                            ),
                            new_alias,
                        )
                    }
                    ColumnPathLink::Leaf(_) => {
                        panic!("Unexpected leaf in dependency link")
                    }
                };

                let join_table_query =
                    from_dependency(dependency, selection_level, database, false);

                let join_table_query = match join_table_query {
                    Table::Physical { table_id, .. } => {
                        Table::physical(table_id, Some(linked_table_alias))
                    }
                    _ => join_table_query,
                };

                Table::Join(LeftJoin::new(acc, join_table_query, join_predicate))
            },
        )
    }

    let table_tree = TableDependency::from_column_path(paths_list).unwrap_or(TableDependency {
        table_id,
        dependencies: vec![],
    });

    from_dependency(table_tree, selection_level, database, true)
}

#[cfg(test)]
mod tests {
    use crate::{
        sql::ExpressionBuilder,
        transform::{pg::selection_level::SelectionLevel, test_util::TestSetup},
        PhysicalColumnPath,
    };

    use multiplatform_test::multiplatform_test;

    #[multiplatform_test]
    fn single_level_join() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_table,
                 concerts_venue_id_column,
                 venues_name_column,
                 ..
             }| {
                // (concert.venue_id, venue.id) -> (venue.name, None)
                let concert_venue_name_path = PhysicalColumnPath::from_columns(
                    vec![concerts_venue_id_column, venues_name_column],
                    &database,
                );

                let join = super::compute_join(
                    concerts_table,
                    &[concert_venue_name_path],
                    &SelectionLevel::TopLevel,
                    &database,
                );

                assert_binding!(
                    join.to_sql(&database),
                    r#""concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
                );
            },
        )
    }

    #[multiplatform_test]
    fn multi_level_join() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_table,

                 concerts_venue_id_column,

                 concert_artists_concert_id_column,
                 concert_artists_artist_id_column,

                 artists_name_column,
                 artists_address_id_column,

                 addresses_city_column,

                 venues_name_column,
                 ..
             }| {
                // (concert.id, concert_artists.concert_id) -> (concert_artists.artist_id, artists.id) -> (artists.name, None)
                let concert_ca_artist_name_path = PhysicalColumnPath::from_columns(
                    vec![
                        concert_artists_concert_id_column,
                        concert_artists_artist_id_column,
                        artists_name_column,
                    ],
                    &database,
                );

                // (concert.id, concert_artists.concert_id) -> (concert_artists.artist_id, artists.id) -> (artists.address_id, address.id) -> (address.city, None)
                let concert_ca_artist_address_path = PhysicalColumnPath::from_columns(
                    vec![
                        concert_artists_concert_id_column,
                        concert_artists_artist_id_column,
                        artists_address_id_column,
                        addresses_city_column,
                    ],
                    &database,
                );

                // (concert.venue_id, venue.id) -> (venue.name, None)
                let concert_venue_path = PhysicalColumnPath::from_columns(
                    vec![concerts_venue_id_column, venues_name_column],
                    &database,
                );

                let join = super::compute_join(
                    concerts_table,
                    &[
                        concert_ca_artist_name_path,
                        concert_ca_artist_address_path,
                        concert_venue_path,
                    ],
                    &SelectionLevel::TopLevel,
                    &database,
                );

                assert_binding!(
                    join.to_sql(&database),
                    r#""concerts" LEFT JOIN "concert_artists" LEFT JOIN "artists" LEFT JOIN "addresses" ON "artists"."address_id" = "addresses"."id" ON "concert_artists"."artist_id" = "artists"."id" ON "concerts"."id" = "concert_artists"."concert_id" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
                );
            },
        )
    }
}
