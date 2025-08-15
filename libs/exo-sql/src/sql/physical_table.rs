// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use crate::{Database, schema::index_spec::IndexKind};

use super::{
    ExpressionBuilder, SQLBuilder, column::Column, delete::Delete, insert::Insert,
    physical_column::PhysicalColumn, predicate::ConcretePredicate, schema_object::SchemaObjectName,
    update::Update,
};

use maybe_owned::MaybeOwned;
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

    pub fn get_sequence_names(&self) -> Vec<SchemaObjectName> {
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
    ) -> Insert<'a>
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

    pub fn delete(&self, predicate: ConcretePredicate, returning: Vec<Column>) -> Delete<'_> {
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
    ) -> Update<'a>
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
