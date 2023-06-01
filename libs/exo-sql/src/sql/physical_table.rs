// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{Database, OneToMany};

use super::{
    column::Column, delete::Delete, insert::Insert, physical_column::PhysicalColumn,
    predicate::ConcretePredicate, update::Update, ExpressionBuilder,
};

use maybe_owned::MaybeOwned;
use serde::{Deserialize, Serialize};

/// A physical table in the database such as "concerts" or "users".
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PhysicalTable {
    /// The name of the table.
    pub name: String,
    /// The columns of the table.
    // concerts.venue_id: (venues.id, "int", "venue_id_table")
    pub columns: Vec<PhysicalColumn>,

    // venues."concerts": (venues.id, concerts.venue_id)
    pub references: Vec<OneToMany>,
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
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    pub fn get_pk_column_index(&self) -> Option<usize> {
        self.columns.iter().position(|c| c.is_pk)
    }

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
            returning,
        }
    }
}

impl ExpressionBuilder for PhysicalTable {
    /// Build a table reference for the `<table>`.
    fn build(&self, _database: &Database, builder: &mut crate::sql::SQLBuilder) {
        builder.push_identifier(&self.name);
    }
}
