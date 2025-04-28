// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{cmp::Ordering, collections::HashSet, hash::Hash};

use crate::{schema::index_spec::IndexKind, Database};

use super::{
    column::Column, delete::Delete, insert::Insert, physical_column::PhysicalColumn,
    predicate::ConcretePredicate, update::Update, ExpressionBuilder, SQLBuilder,
};

use maybe_owned::MaybeOwned;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, Clone)]
pub struct PhysicalTableName {
    /// The name of the table.
    pub name: String,
    /// The schema of the table.
    pub schema: Option<String>,
}

impl PartialOrd for PhysicalTableName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PhysicalTableName {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.name, self.schema.as_deref()).cmp(&(&other.name, other.schema.as_deref()))
    }
}

impl PartialEq for PhysicalTableName {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && match (self.schema.as_deref(), other.schema.as_deref()) {
                (Some(s1), Some(s2)) => s1 == s2,
                (None, None) | (Some("public"), None) | (None, Some("public")) => true,
                _ => false,
            }
    }
}

impl Hash for PhysicalTableName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        match &self.schema {
            Some(schema) if schema != "public" => schema.hash(state),
            _ => (),
        }
    }
}

impl PhysicalTableName {
    pub fn new(name: impl Into<String>, schema: Option<&str>) -> Self {
        Self {
            name: name.into(),
            schema: match schema {
                Some(schema) if schema != "public" => Some(schema.to_string()),
                _ => None,
            },
        }
    }

    pub fn new_with_schema_name(name: impl Into<String>, schema_name: impl Into<String>) -> Self {
        let schema_name = schema_name.into();

        Self {
            name: name.into(),
            schema: match schema_name.as_str() {
                "public" => None,
                _ => Some(schema_name),
            },
        }
    }

    pub fn fully_qualified_name(&self) -> String {
        self.fully_qualified_name_with_sep(".")
    }

    pub fn fully_qualified_name_with_sep(&self, sep: &str) -> String {
        match &self.schema {
            Some(schema) => format!("{}{}{}", schema, sep, self.name),
            None => self.name.to_owned(),
        }
    }

    pub(crate) fn synthetic_name(&self) -> String {
        match &self.schema {
            Some(schema) => format!("{}#{}", schema, self.name),
            None => self.name.to_owned(),
        }
    }

    pub fn sql_name(&self) -> String {
        match self.schema {
            Some(ref schema) => format!("\"{}\".\"{}\"", schema, self.name),
            None => format!("\"{}\"", self.name),
        }
    }

    pub fn schema_name(&self) -> String {
        match self.schema {
            Some(ref schema) => schema.to_string(),
            None => "public".to_string(),
        }
    }
}

/// A physical table in the database such as "concerts" or "users".
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct PhysicalTable {
    pub name: PhysicalTableName,
    /// The columns of the table.
    // concerts.venue_id: (venues.id, "int", "venue_id_table")
    pub columns: Vec<PhysicalColumn>,

    pub indices: Vec<PhysicalIndex>,

    pub managed: bool,
}

/// A physical enum in the database such as "Priority" with variants "LOW", "MEDIUM", "HIGH".
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct PhysicalEnum {
    pub name: PhysicalTableName,
    pub variants: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PhysicalIndex {
    pub name: String,
    pub columns: HashSet<String>,
    pub index_kind: IndexKind,
}

/// The derived implementation of `Debug` is quite verbose, so we implement it manually
/// to print the table and columns names only.
impl std::fmt::Debug for PhysicalTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Table: ")?;
        if let Some(schema) = &self.name.schema {
            f.write_str(schema)?;
            f.write_str(".")?;
        }
        f.write_str(&self.name.name)?;
        f.write_str(", Columns: [")?;
        for column in &self.columns {
            f.write_str(&column.name)?;
            f.write_str(", ")?;
        }
        f.write_str("]")?;
        Ok(())
    }
}

impl PhysicalTable {
    pub fn get_pk_physical_columns(&self) -> Vec<&PhysicalColumn> {
        self.columns.iter().filter(|column| column.is_pk).collect()
    }

    pub fn get_sequence_names(&self) -> Vec<PhysicalTableName> {
        self.columns
            .iter()
            .flat_map(|column| column.get_sequence_name())
            .collect()
    }

    pub fn insert<'a, C>(
        &'a self,
        columns: Vec<&'a PhysicalColumn>,
        column_values_seq: Vec<Vec<C>>,
        returning: Vec<MaybeOwned<'a, Column>>,
    ) -> Insert
    where
        C: Into<MaybeOwned<'a, Column>>,
    {
        Insert {
            table: self,
            columns,
            values_seq: column_values_seq
                .into_iter()
                .map(|rows| rows.into_iter().map(|col| col.into()).collect())
                .collect(),
            returning,
        }
    }

    pub fn delete(&self, predicate: ConcretePredicate, returning: Vec<Column>) -> Delete {
        Delete {
            table: self,
            predicate: predicate.into(),
            additional_predicate: None,
            returning: returning.into_iter().map(|col| col.into()).collect(),
        }
    }

    pub fn update<'a, C>(
        &'a self,
        column_values: Vec<(&'a PhysicalColumn, C)>,
        predicate: MaybeOwned<'a, ConcretePredicate>,
        returning: Vec<MaybeOwned<'a, Column>>,
    ) -> Update
    where
        C: Into<MaybeOwned<'a, Column>>,
    {
        Update {
            table: self,
            column_values: column_values
                .into_iter()
                .map(|(pc, col)| (pc, col.into()))
                .collect(),
            predicate,
            additional_predicate: None,
            returning,
        }
    }

    pub(crate) fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    pub(crate) fn get_pk_column_indices(&self) -> Vec<usize> {
        self.columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_pk)
            .map(|(i, _)| i)
            .collect()
    }
}

impl ExpressionBuilder for PhysicalTable {
    /// Build a table reference for the `<table>`.
    fn build(&self, _database: &Database, builder: &mut SQLBuilder) {
        builder.push_table(&self.name);
    }
}
