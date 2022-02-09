use std::collections::{hash_map::Entry, HashMap};

use crate::sql::{column::Column, predicate::Predicate, PhysicalTable, TableQuery};

use super::column_path::ColumnPathLink;

/// Compute TableJoin from a list of column paths
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
pub fn compute_join<'a>(
    table: &'a PhysicalTable,
    links_list: Vec<Vec<ColumnPathLink<'a>>>,
) -> TableQuery<'a> {
    if links_list.is_empty() {
        return TableQuery::Physical(table);
    }

    // Use a stable hasher (FxBuildHasher) so that our test assertions work
    // The kind of workload we're trying to model here is not sensitive to the
    // DOS attack (we control all columns and tables), so we can use a stable hasher
    let mut grouped: HashMap<ColumnPathLink, Vec<Vec<ColumnPathLink>>, _> =
        HashMap::with_hasher(fxhash::FxBuildHasher::default());

    for mut links in links_list {
        if links.is_empty() {
            panic!("Invalid paths list")
        }
        let head = links.remove(0);
        let tail = links;

        if head.linked_column.is_some() {
            let existing = grouped.entry(head);

            match existing {
                Entry::Occupied(mut entry) => entry.get_mut().push(tail),
                Entry::Vacant(entry) => {
                    entry.insert(vec![tail]);
                }
            }
        }
    }

    grouped.into_iter().fold(
        TableQuery::Physical(table),
        |acc, (head_link, tail_links)| {
            let join_predicate = Predicate::Eq(
                Column::Physical(head_link.self_column.0).into(),
                Column::Physical(head_link.linked_column.unwrap().0).into(),
            );

            let join_table_query = compute_join(head_link.linked_column.unwrap().1, tail_links);

            acc.join(join_table_query, join_predicate.into())
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        asql::{column_path::ColumnPathLink, test_util::TestSetup},
        sql::{Expression, ExpressionContext},
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
                    r#""concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id" LEFT JOIN "concert_artists" LEFT JOIN "artists" LEFT JOIN "addresses" ON "artists"."address_id" = "addresses"."id" ON "concert_artists"."artist_id" = "artists"."id" ON "concerts"."id" = "concert_artists"."concert_id""#
                );
            },
        )
    }
}
