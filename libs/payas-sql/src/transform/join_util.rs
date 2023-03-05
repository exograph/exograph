use crate::{
    asql::column_path::ColumnPathLink,
    sql::{column::Column, predicate::ConcretePredicate, table::TableQuery},
    transform::table_dependency::{DependencyLink, TableDependency},
    PhysicalTable,
};

pub fn compute_join<'a>(
    table: &'a PhysicalTable,
    paths_list: Vec<Vec<ColumnPathLink<'a>>>,
) -> TableQuery<'a> {
    fn from_dependency(dependency: TableDependency) -> TableQuery {
        dependency.dependencies.into_iter().fold(
            TableQuery::Physical(dependency.table),
            |acc, DependencyLink { link, dependency }| {
                let join_predicate = ConcretePredicate::Eq(
                    Column::Physical(link.self_column.0),
                    Column::Physical(link.linked_column.unwrap().0),
                );

                let join_table_query = from_dependency(dependency);

                acc.join(join_table_query, join_predicate.into())
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
        asql::column_path::ColumnPathLink,
        sql::{Expression, ExpressionContext},
        transform::test_util::TestSetup,
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

                let join = super::compute_join(concerts_table, vec![concert_venue]);

                let mut expr = ExpressionContext::default();
                let join_binding = join.binding(&mut expr);
                assert_binding!(
                    join_binding,
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
                    vec![concert_ca_artist, concert_ca_artist_address, concert_venue],
                );

                let mut expr = ExpressionContext::default();
                let join_binding = join.binding(&mut expr);
                assert_binding!(
                    join_binding,
                    r#""concerts" LEFT JOIN "concert_artists" LEFT JOIN "artists" LEFT JOIN "addresses" ON "artists"."address_id" = "addresses"."id" ON "concert_artists"."artist_id" = "artists"."id" ON "concerts"."id" = "concert_artists"."concert_id" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
                );
            },
        )
    }
}
