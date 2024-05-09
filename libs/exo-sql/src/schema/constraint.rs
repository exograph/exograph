// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    database_error::DatabaseError, sql::connect::database_client::DatabaseClient, PhysicalTableName,
};

pub(super) struct PrimaryKeyConstraint {
    pub(super) _constraint_name: String,
    pub(super) columns: HashSet<String>,
}

pub(super) struct ForeignKeyConstraint {
    pub(super) _constraint_name: String,
    pub(super) self_columns: HashSet<String>,
    pub(super) foreign_table: PhysicalTableName,
    pub(super) foreign_columns: HashSet<String>,
}

#[derive(Debug)]
pub(super) struct UniqueConstraint {
    pub(super) constraint_name: String,
    pub(super) columns: HashSet<String>,
}

pub(super) struct Constraints {
    pub(super) primary_key: PrimaryKeyConstraint,
    pub(super) foreign_constraints: Vec<ForeignKeyConstraint>,
    pub(super) uniques: Vec<UniqueConstraint>,
}

lazy_static! {
    static ref PRIMARY_KEY_RE: Regex = Regex::new(r"PRIMARY KEY \(([^)]+)\)").unwrap();
    static ref FOREIGN_KEY_RE: Regex =
        Regex::new(r"FOREIGN KEY \(([^)]+)\) REFERENCES ([^\(]+)\(([^)]+)\)").unwrap();
    static ref UNIQUE_RE: Regex = Regex::new(r"UNIQUE \(([^)]+)\)").unwrap();
    static ref LIST_RE: Regex = Regex::new(r"(\w+)").unwrap();
}

impl Constraints {
    pub(super) async fn from_live_db(
        client: &DatabaseClient,
        table_name: &PhysicalTableName,
    ) -> Result<Constraints, DatabaseError> {
        // Query to get a list of constraints in the table (primary key and foreign key constraints)
        let constraints_query = format!(
            "
            SELECT contype, conname, pg_get_constraintdef(oid, true) as condef
            FROM pg_constraint
            WHERE conrelid = '{}'::regclass AND conparentid = 0",
            table_name.fully_qualified_name()
        );

        // Get all the constraints in the table
        let constraints = client
            .query(constraints_query.as_str(), &[])
            .await?
            .iter()
            .map(|row| {
                let contype: i8 = row.get("contype");
                let conname: String = row.get("conname");
                let condef: String = row.get("condef");

                (contype as u8 as char, conname, condef)
            })
            .collect::<Vec<_>>();

        // Filter out primary key constraints to find which columns are primary keys
        let primary_key = constraints
            .iter()
            .filter(|(contype, _, _)| *contype == 'p')
            .map(|(_, conname, condef)| {
                let matches = PRIMARY_KEY_RE.captures_iter(condef).next().unwrap();
                let columns = Self::parse_column_list(&matches[1]);
                PrimaryKeyConstraint {
                    _constraint_name: conname.to_string(),
                    columns,
                }
            })
            .next()
            .unwrap();

        // Filter out foreign key constraints to find which columns require foreign key constraints
        let foreign_constraints = constraints
            .iter()
            .filter(|(contype, _, _)| *contype == 'f')
            .map(|(_, conname, condef)| {
                let matches = FOREIGN_KEY_RE.captures_iter(condef).next().unwrap();
                let self_columns = Self::parse_column_list(&matches[1]); // name of the column
                let foreign_table = matches[2].to_owned(); // name of the table the column refers to
                let foreign_columns = Self::parse_column_list(&matches[3]); // name of the column in the referenced table

                ForeignKeyConstraint {
                    _constraint_name: conname.to_string(),
                    self_columns,
                    foreign_table: PhysicalTableName {
                        name: foreign_table,
                        schema: None,
                    },
                    foreign_columns,
                }
            })
            .collect::<Vec<_>>();

        let uniques = constraints
            .iter()
            .filter(|(contype, _, _)| *contype == 'u')
            .map(|(_, conname, condef)| {
                let matches = UNIQUE_RE.captures_iter(condef).next().unwrap();
                let columns = Self::parse_column_list(&matches[1]);
                UniqueConstraint {
                    constraint_name: conname.to_string(),
                    columns,
                }
            })
            .collect();

        Ok(Constraints {
            primary_key,
            foreign_constraints,
            uniques,
        })
    }

    fn parse_column_list(column_list: &str) -> HashSet<String> {
        // Basically just split the string on commas and remove the quotes (the regex takes care of the quotes)
        LIST_RE
            .captures_iter(column_list)
            .flat_map(|c| {
                c.iter()
                    .skip(1) // Skip the first that is the entire match
                    .flat_map(|m| m.map(|m| m.as_str().to_owned()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

/// Returns a comma separated list of the items in the set, sorted alphabetically If `quote_name` is
/// true, then the items will be quoted Useful when generating SQL unique constraints, where columns
/// provided will be a set but we need to generate a stable string to compare against the existing
/// constraints
pub(super) fn sorted_comma_list<T: ToString>(list: &HashSet<T>, quote_name: bool) -> String {
    let mut list = list
        .iter()
        .map(|item| {
            if quote_name {
                format!("\"{}\"", item.to_string())
            } else {
                item.to_string()
            }
        })
        .collect::<Vec<_>>();
    list.sort();
    list.join(", ")
}
