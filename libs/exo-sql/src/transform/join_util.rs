// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    Database, PhysicalColumnPath, TableId,
    asql::column_path::{ColumnPathLink, RelationLink},
    sql::{column::Column, join::LeftJoin, predicate::ConcretePredicate, table::Table},
    transform::table_dependency::{DependencyLink, TableDependency},
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
        forced_alias: Option<String>,
    ) -> Table {
        // We don't use the alias for the top level table in predicate, so match the behavior here
        let init_table = {
            let alias = forced_alias.or_else(|| {
                if top_level {
                    selection_level.self_referencing_table_alias(dependency.table_id, database)
                } else {
                    Some(selection_level.alias((dependency.table_id, None), database))
                }
            });

            Table::physical(dependency.table_id, alias)
        };

        let current_alias = match &init_table {
            Table::Physical { alias, .. } => alias.clone(),
            _ => None,
        };

        dependency.dependencies.into_iter().fold(
            init_table,
            |acc, DependencyLink { link, dependency }| {
                let (join_predicate, linked_table_alias) = match link {
                    ColumnPathLink::Relation(relation_link) => {
                        let linked_table_id = relation_link.linked_table_id;
                        let RelationLink {
                            column_pairs,
                            linked_table_alias,
                            ..
                        } = relation_link;

                        let new_alias =
                            selection_level.alias((linked_table_id, linked_table_alias), database);

                        let predicate = column_pairs.iter().fold(
                            ConcretePredicate::True,
                            |acc, column_pair| {
                                ConcretePredicate::and(
                                    acc,
                                    ConcretePredicate::Eq(
                                        Column::physical(
                                            column_pair.self_column_id,
                                            current_alias.clone(),
                                        ),
                                        Column::physical(
                                            column_pair.foreign_column_id,
                                            Some(new_alias.clone()),
                                        ),
                                    ),
                                )
                            },
                        );
                        (predicate, new_alias)
                    }
                    ColumnPathLink::Leaf(_) => {
                        panic!("Unexpected leaf in dependency link")
                    }
                };

                let join_table_query = from_dependency(
                    dependency,
                    selection_level,
                    database,
                    false,
                    Some(linked_table_alias.clone()),
                );

                Table::Join(Box::new(LeftJoin::new(
                    acc,
                    join_table_query,
                    join_predicate,
                )))
            },
        )
    }

    let table_tree = TableDependency::from_column_path(paths_list).unwrap_or(TableDependency {
        table_id,
        dependencies: vec![],
    });

    from_dependency(table_tree, selection_level, database, true, None)
}

#[cfg(test)]
mod tests {
    use crate::{
        PhysicalColumnPath,
        sql::ExpressionBuilder,
        transform::{pg::selection_level::SelectionLevel, test_util::TestSetup},
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
