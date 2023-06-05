// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Abstraction to allow inserting rows into a table as well as related tables.
//!
//! This allows us to execute GraphQL mutations like this:
//! ```graphql
//! mutation {
//!   createVenue(data: {name: "v1", published: true, latitude: 1.2, concerts: [
//!     {title: "c1", published: true, price: 1.2},
//!     {title: "c2", published: false, price: 2.4}
//!   ]}) {
//!     id
//!   }
//! }
//! ```
//!
//! Here, concerts created will have their `venue_id` set to the id of the venue being created.

use super::select::AbstractSelect;
use crate::sql::column::Column;
use crate::{ColumnId, OneToManyRelationId, TableId};

#[derive(Debug)]
pub struct AbstractInsert {
    /// Table to insert into
    pub table_id: TableId,
    /// Rows to insert
    pub rows: Vec<InsertionRow>,
    /// The selection to return
    pub selection: AbstractSelect,
}

/// A logical row to be inserted (see `InsertionElement` for more details).
#[derive(Debug)]
pub struct InsertionRow {
    pub elems: Vec<InsertionElement>,
}

impl InsertionRow {
    /// Partitions the elements into two groups: those that are inserted into
    /// the table itself, and those that are inserted into nested tables.
    pub fn partition_self_and_nested(&self) -> (Vec<&ColumnValuePair>, Vec<&NestedInsertion>) {
        let mut self_elems = Vec::new();
        let mut nested_elems = Vec::new();
        for elem in &self.elems {
            match elem {
                InsertionElement::SelfInsert(pair) => self_elems.push(pair),
                InsertionElement::NestedInsert(nested) => nested_elems.push(nested),
            }
        }
        (self_elems, nested_elems)
    }
}

#[derive(Debug)]
pub struct NestedInsertion {
    /// The relation with the parent element (the self_pk_column_id is the parent table's pk column and the self_column_id is the column in the table being inserted that refers to the the parent table)
    pub relation_id: OneToManyRelationId,
    pub insertions: Vec<InsertionRow>,
}

/// A pair of column and value to be inserted into the table.
#[derive(Debug)]
pub struct ColumnValuePair {
    pub column: ColumnId,
    pub value: Column,
}

impl ColumnValuePair {
    pub fn new(column: ColumnId, value: Column) -> Self {
        Self { column, value }
    }
}

/// Logical element to be inserted. Each element could be thought of as an
/// attribute of the logical document. For example, with `Venue <-> [Concert]`
/// model, logical elements in `Venue` include its own columns (name,
/// address, etc.), which would be represented by the `SelfInsert` variant. It
/// also includes the logically nested "concerts" element, which would be
/// represented by the `NestedInsert` variant.
#[derive(Debug)]
pub enum InsertionElement {
    /// Value to be inserted into the table itself
    SelfInsert(ColumnValuePair),
    /// Value to be inserted into a nested tables
    NestedInsert(NestedInsertion),
}
