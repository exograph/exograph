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

use maybe_owned::MaybeOwned;

use super::select::AbstractSelect;
use super::selection::NestedElementRelation;
use crate::sql::column::Column;
use crate::{ColumnId, TableId};

#[derive(Debug)]
pub struct AbstractInsert<'a> {
    /// Table to insert into
    pub table_id: TableId,
    /// Rows to insert
    pub rows: Vec<InsertionRow<'a>>,
    /// The selection to return
    pub selection: AbstractSelect<'a>,
}

/// A logical row to be inserted (see `InsertionElement` for more details).
#[derive(Debug)]
pub struct InsertionRow<'a> {
    pub elems: Vec<InsertionElement<'a>>,
}

impl<'a> InsertionRow<'a> {
    /// Partitions the elements into two groups: those that are inserted into
    /// the table itself, and those that are inserted into nested tables.
    pub fn partition_self_and_nested(
        &'a self,
    ) -> (Vec<&'a ColumnValuePair<'a>>, Vec<&'a NestedInsertion<'a>>) {
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
pub struct NestedInsertion<'a> {
    /// The parent table (for example the `venues` table in `Venue <-> [Concert]`)
    pub parent_table: TableId,
    /// Relation between the parent table and the nested table (column: concerts.venue_id, table: concerts)
    pub relation: NestedElementRelation,
    /// The insertions to be performed on the nested table ([{title: "c1", published: true, price: 1.2}, {title: "c2", published: false, price: 2.4}]}])
    pub insertions: Vec<InsertionRow<'a>>,
}

/// A pair of column and value to be inserted into the table.
#[derive(Debug)]
pub struct ColumnValuePair<'a> {
    pub column: ColumnId,
    pub value: MaybeOwned<'a, Column<'a>>,
}

impl<'a> ColumnValuePair<'a> {
    pub fn new(column: ColumnId, value: MaybeOwned<'a, Column<'a>>) -> Self {
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
pub enum InsertionElement<'a> {
    /// Value to be inserted into the table itself
    SelfInsert(ColumnValuePair<'a>),
    /// Value to be inserted into a nested tables
    NestedInsert(NestedInsertion<'a>),
}
