// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(test)]

use crate::schema::database_spec::test_helper::{
    int_column, pk_column, pk_reference_column, string_column,
};
use crate::schema::database_spec::{DatabaseSpec, TableSpec};
use crate::{ColumnId, Database, TableId};

pub struct TestSetup {
    pub database: Database,

    pub concerts_table: TableId,
    pub concert_artists_table: TableId,
    pub artists_table: TableId,
    pub addresses_table: TableId,
    pub venues_table: TableId,

    pub concerts_id_column: ColumnId,
    pub concerts_name_column: ColumnId,
    pub concerts_venue_id_column: ColumnId,

    pub concert_artists_concert_id_column: ColumnId,
    pub concert_artists_artist_id_column: ColumnId,

    pub artists_id_column: ColumnId,
    pub artists_name_column: ColumnId,
    pub artists_address_id_column: ColumnId,

    pub addresses_id_column: ColumnId,
    pub addresses_city_column: ColumnId,

    pub venues_id_column: ColumnId,
    pub venues_name_column: ColumnId,
}

impl TestSetup {
    pub fn with_setup(test_fn: impl Fn(TestSetup)) {
        let database = DatabaseSpec::new(vec![
            TableSpec::new(
                "concerts",
                vec![
                    pk_column("id"),
                    int_column("venue_id"),
                    string_column("name"),
                ],
            ),
            TableSpec::new(
                "venues",
                vec![
                    pk_column("id"),
                    string_column("name"),
                    pk_reference_column("artist_id", "artists"),
                ],
            ),
            TableSpec::new(
                "concert_artists",
                vec![
                    pk_column("id"),
                    pk_reference_column("concert_id", "concerts"),
                    pk_reference_column("artist_id", "artists"),
                ],
            ),
            TableSpec::new(
                "artists",
                vec![
                    pk_column("id"),
                    string_column("name"),
                    pk_reference_column("address_id", "addresses"),
                ],
            ),
            TableSpec::new("addresses", vec![pk_column("id"), string_column("city")]),
        ])
        .to_database();

        let concert_table_id = database.get_table_id("concerts").unwrap();

        let concerts_id_column = database.get_column_id(concert_table_id, "id").unwrap();
        let concerts_name_column = database.get_column_id(concert_table_id, "name").unwrap();
        let concerts_venue_id_column = database
            .get_column_id(concert_table_id, "venue_id")
            .unwrap();

        let venues_table_id = database.get_table_id("venues").unwrap();
        let venues_id_column = database.get_column_id(venues_table_id, "id").unwrap();
        let venues_name_column = database.get_column_id(venues_table_id, "name").unwrap();

        let concert_artists_table_id = database.get_table_id("concert_artists").unwrap();
        let _concert_artists_id_column = database
            .get_column_id(concert_artists_table_id, "id")
            .unwrap();
        let concert_artists_concert_id_column = database
            .get_column_id(concert_artists_table_id, "concert_id")
            .unwrap();
        let concert_artists_artist_id_column = database
            .get_column_id(concert_artists_table_id, "artist_id")
            .unwrap();

        let artists_table_id = database.get_table_id("artists").unwrap();
        let artists_id_column = database.get_column_id(artists_table_id, "id").unwrap();
        let artists_name_column = database.get_column_id(artists_table_id, "name").unwrap();
        let artists_address_id_column = database
            .get_column_id(artists_table_id, "address_id")
            .unwrap();

        let addresses_table_id = database.get_table_id("addresses").unwrap();
        let addresses_id_column = database.get_column_id(addresses_table_id, "id").unwrap();
        let addresses_city_column = database.get_column_id(addresses_table_id, "city").unwrap();

        let test_setup = TestSetup {
            database,
            concerts_table: concert_table_id,
            concert_artists_table: concert_artists_table_id,
            artists_table: artists_table_id,
            addresses_table: addresses_table_id,
            venues_table: venues_table_id,

            concerts_id_column,
            concerts_name_column,
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
        };

        test_fn(test_setup)
    }
}
