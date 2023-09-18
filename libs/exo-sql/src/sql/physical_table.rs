// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::Database;

use super::{
    column::Column, delete::Delete, insert::Insert, physical_column::PhysicalColumn,
    predicate::ConcretePredicate, update::Update, ExpressionBuilder, SQLBuilder,
};

use maybe_owned::MaybeOwned;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq)]
pub struct PhysicalTableName {
    pub name: String,
    pub schema: Option<String>,
}

/// A physical table in the database such as "concerts" or "users".
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PhysicalTable {
    /// The name of the table.
    pub name: String,
    /// The schema of the table.
    pub schema: Option<String>,
    /// The columns of the table.
    // concerts.venue_id: (venues.id, "int", "venue_id_table")
    pub columns: Vec<PhysicalColumn>,
}

/// The derived implementation of `Debug` is quite verbose, so we implement it manually
/// to print the table name only.
impl std::fmt::Debug for PhysicalTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Table: ")?;
        f.write_str(&self.name)
    }
}

impl PhysicalTable {
    pub fn get_pk_physical_column(&self) -> Option<&PhysicalColumn> {
        self.columns.iter().find(|column| column.is_pk)
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

    pub(crate) fn get_pk_column_index(&self) -> Option<usize> {
        self.columns.iter().position(|c| c.is_pk)
    }

    pub(crate) fn physical_name(&self) -> PhysicalTableName {
        PhysicalTableName {
            name: self.name.to_owned(),
            schema: self.schema.to_owned(),
        }
    }
}

impl ExpressionBuilder for PhysicalTable {
    /// Build a table reference for the `<table>`.
    fn build(&self, _database: &Database, builder: &mut SQLBuilder) {
        builder.push_table(&self.name, self.schema.as_ref());
    }
}
