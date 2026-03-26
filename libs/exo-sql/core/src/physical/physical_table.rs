// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use crate::index_kind::PhysicalIndexKind;
use crate::physical_column::PhysicalColumn;
use crate::schema_object::SchemaObjectName;

use serde::{Deserialize, Serialize};

/// A physical table in the database such as "concerts" or "users".
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct PhysicalTable {
    pub name: SchemaObjectName,
    /// The columns of the table.
    // concerts.venue_id: (venues.id, "int", "venue_id_table")
    pub columns: Vec<PhysicalColumn>,

    pub indices: Vec<PhysicalIndex>,

    pub managed: bool,
}

/// A physical enum in the database such as "Priority" with variants "LOW", "MEDIUM", "HIGH".
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct PhysicalEnum {
    pub name: SchemaObjectName,
    pub variants: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PhysicalIndex {
    pub name: String,
    pub columns: HashSet<String>,
    pub index_kind: Box<dyn PhysicalIndexKind>,
}

impl PartialEq for PhysicalIndex {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.columns == other.columns
            && self.index_kind.equals(other.index_kind.as_ref())
    }
}

impl Eq for PhysicalIndex {}

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

    pub fn get_sequence_names(&self) -> Vec<SchemaObjectName> {
        self.columns
            .iter()
            .flat_map(|column| column.get_sequence_name())
            .collect()
    }

    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    pub fn get_pk_column_indices(&self) -> Vec<usize> {
        self.columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_pk)
            .map(|(i, _)| i)
            .collect()
    }
}
