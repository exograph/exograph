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
    SchemaObjectName, database_error::DatabaseError, sql::connect::database_client::DatabaseClient,
};

pub(super) struct PrimaryKeyConstraint {
    pub(super) _constraint_name: String,
    pub(super) columns: Vec<String>,
}

pub(super) struct ForeignKeyConstraintColumnPair {
    pub(super) self_column: String,
    pub(super) foreign_column: String,
}

pub(super) struct ForeignKeyConstraint {
    pub(super) constraint_name: String,
    pub(super) column_pairs: Vec<ForeignKeyConstraintColumnPair>,
    pub(super) foreign_table: SchemaObjectName,
}

#[derive(Debug)]
pub(super) struct UniqueConstraint {
    pub(super) constraint_name: String,
    pub(super) columns: Vec<String>,
}

pub(super) struct Constraints {
    pub(super) primary_key: Option<PrimaryKeyConstraint>,
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

const CONSTRAINT_QUERY: &str = "
SELECT 
    contype, 
    conname, 
    pg_get_constraintdef(pg_constraint.oid, true) AS condef, 
    pg_class.relname AS foreign_table, 
    pg_namespace.nspname AS foreign_schema
FROM 
    pg_constraint 
    LEFT JOIN pg_class AS self_table ON pg_constraint.conrelid = self_table.oid 
    LEFT JOIN pg_namespace AS self_schema ON self_table.relnamespace = self_schema.oid
    LEFT JOIN pg_class ON pg_constraint.confrelid = pg_class.oid 
    LEFT JOIN pg_namespace ON pg_class.relnamespace = pg_namespace.oid
WHERE 
    self_table.relname = $1 
    AND self_schema.nspname = $2 
    AND conparentid = 0;
";

impl Constraints {
    pub(super) async fn from_live_db(
        client: &DatabaseClient,
        table_name: &SchemaObjectName,
    ) -> Result<Constraints, DatabaseError> {
        // Get a list of constraints in the table (primary key and foreign key constraints)
        let constraints = client
            .query(
                CONSTRAINT_QUERY,
                &[&table_name.name, &table_name.schema_name()],
            )
            .await?
            .iter()
            .map(|row| {
                let contype: i8 = row.get("contype");
                let conname: String = row.get("conname");
                let condef: String = row.get("condef");
                let foreign_table: Option<String> = row.get("foreign_table");
                let foreign_schema: Option<String> = row.get("foreign_schema");
                (
                    contype as u8 as char,
                    conname,
                    condef,
                    foreign_table,
                    foreign_schema,
                )
            })
            .collect::<Vec<_>>();

        // Filter out primary key constraints to find which columns are primary keys
        let primary_key = constraints
            .iter()
            .filter(|(contype, _, _, _, _)| *contype == 'p')
            .map(|(_, conname, condef, _, _)| {
                let matches = PRIMARY_KEY_RE.captures_iter(condef).next().unwrap();
                let columns = Self::parse_column_list(&matches[1]);
                PrimaryKeyConstraint {
                    _constraint_name: conname.to_string(),
                    columns,
                }
            })
            .next();

        // Filter out foreign key constraints to find which columns require foreign key constraints
        let foreign_constraints = constraints
            .iter()
            .filter(|(contype, _, _, _, _)| *contype == 'f')
            .map(|(_, conname, condef, foreign_table, foreign_schema)| {
                let foreign_table = SchemaObjectName {
                    name: foreign_table.clone().unwrap(),
                    schema: match foreign_schema {
                        Some(schema) if schema != "public" => Some(schema.to_string()),
                        _ => None,
                    },
                };

                let matches = FOREIGN_KEY_RE.captures_iter(condef).next().unwrap();
                let self_columns = Self::parse_column_list(&matches[1]); // name of the column
                let foreign_columns = Self::parse_column_list(&matches[3]); // name of the column in the referenced table

                ForeignKeyConstraint {
                    constraint_name: conname.to_string(),
                    column_pairs: self_columns
                        .into_iter()
                        .zip(foreign_columns.into_iter())
                        .map(
                            |(self_column, foreign_column)| ForeignKeyConstraintColumnPair {
                                self_column,
                                foreign_column,
                            },
                        )
                        .collect(),
                    foreign_table,
                }
            })
            .collect::<Vec<_>>();

        let uniques = constraints
            .iter()
            .filter(|(contype, _, _, _, _)| *contype == 'u')
            .map(|(_, conname, condef, _, _)| {
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

    fn parse_column_list(column_list: &str) -> Vec<String> {
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
