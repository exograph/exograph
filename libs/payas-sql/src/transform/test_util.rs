#![cfg(test)]

use crate::{
    sql::physical_column::{IntBits, PhysicalColumn, PhysicalColumnType},
    PhysicalTable,
};

pub struct TestSetup<'a> {
    pub concerts_table: &'a PhysicalTable,
    pub concert_artists_table: &'a PhysicalTable,
    pub artists_table: &'a PhysicalTable,
    pub addresses_table: &'a PhysicalTable,
    pub venues_table: &'a PhysicalTable,

    pub concerts_id_column: &'a PhysicalColumn,
    pub concerts_name_column: &'a PhysicalColumn,
    pub concerts_venue_id_column: &'a PhysicalColumn,

    pub concert_artists_concert_id_column: &'a PhysicalColumn,
    pub concert_artists_artist_id_column: &'a PhysicalColumn,

    pub artists_id_column: &'a PhysicalColumn,
    pub artists_name_column: &'a PhysicalColumn,
    pub artists_address_id_column: &'a PhysicalColumn,

    pub addresses_id_column: &'a PhysicalColumn,
    pub addresses_city_column: &'a PhysicalColumn,

    pub venues_id_column: &'a PhysicalColumn,
    pub venues_name_column: &'a PhysicalColumn,
}

impl TestSetup<'_> {
    pub fn with_setup(test_fn: impl Fn(&TestSetup)) {
        let concerts_table = &PhysicalTable {
            name: "concerts".to_string(),
            columns: vec![
                PhysicalColumn {
                    table_name: "concerts".to_string(),
                    name: "id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: true,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
                PhysicalColumn {
                    table_name: "concerts".to_string(),
                    name: "name".to_string(),
                    typ: PhysicalColumnType::String { length: None },
                    is_pk: false,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
                PhysicalColumn {
                    table_name: "concerts".to_string(),
                    name: "venue_id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: false,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
            ],
        };

        let concerts_id_column = concerts_table.get_physical_column("id").unwrap();
        let concerts_name_column = concerts_table.get_physical_column("name").unwrap();
        let concerts_venue_id_column = concerts_table.get_physical_column("venue_id").unwrap();

        let venues_table = &PhysicalTable {
            name: "venues".to_string(),
            columns: vec![
                PhysicalColumn {
                    table_name: "venues".to_string(),
                    name: "id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: true,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
                PhysicalColumn {
                    table_name: "venues".to_string(),
                    name: "name".to_string(),
                    typ: PhysicalColumnType::String { length: None },
                    is_pk: false,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
            ],
        };

        let venues_id_column = venues_table.get_physical_column("id").unwrap();
        let venues_name_column = venues_table.get_physical_column("name").unwrap();

        let concert_artists_table = &PhysicalTable {
            name: "concert_artists".to_string(),
            columns: vec![
                PhysicalColumn {
                    table_name: "concert_artists".to_string(),
                    name: "id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: true,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
                PhysicalColumn {
                    table_name: "concert_artists".to_string(),
                    name: "concert_id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: false,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
                PhysicalColumn {
                    table_name: "concert_artists".to_string(),
                    name: "artist_id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: false,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
            ],
        };

        let _concert_artists_id_column = concert_artists_table.get_physical_column("id").unwrap();
        let concert_artists_concert_id_column = concert_artists_table
            .get_physical_column("concert_id")
            .unwrap();
        let concert_artists_artist_id_column = concert_artists_table
            .get_physical_column("artist_id")
            .unwrap();

        let artists_table = &PhysicalTable {
            name: "artists".to_string(),
            columns: vec![
                PhysicalColumn {
                    table_name: "artists".to_string(),
                    name: "id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: true,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
                PhysicalColumn {
                    table_name: "artists".to_string(),
                    name: "name".to_string(),
                    typ: PhysicalColumnType::String { length: None },
                    is_pk: false,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
                PhysicalColumn {
                    table_name: "artists".to_string(),
                    name: "address_id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: false,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
            ],
        };

        let artists_id_column = artists_table.get_physical_column("id").unwrap();
        let artists_name_column = artists_table.get_physical_column("name").unwrap();
        let artists_address_id_column = artists_table.get_physical_column("address_id").unwrap();

        let addresses_table = &PhysicalTable {
            name: "addresses".to_string(),
            columns: vec![
                PhysicalColumn {
                    table_name: "addresses".to_string(),
                    name: "id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: true,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
                PhysicalColumn {
                    table_name: "addresses".to_string(),
                    name: "city".to_string(),
                    typ: PhysicalColumnType::String { length: None },
                    is_pk: false,
                    is_auto_increment: false,
                    is_nullable: true,
                    unique_constraints: vec![],
                    default_value: None,
                },
            ],
        };

        let addresses_id_column = addresses_table.get_physical_column("id").unwrap();
        let addresses_city_column = addresses_table.get_physical_column("city").unwrap();

        let test_setup = TestSetup {
            concerts_table,
            concert_artists_table,
            artists_table,
            addresses_table,
            venues_table,

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

        test_fn(&test_setup)
    }
}
