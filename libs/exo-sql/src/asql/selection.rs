// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Support for selecting columns in a table, including json aggregates

use crate::{ColumnId, RelationId, sql::function::Function};

use super::select::AbstractSelect;

/// A selection element along with its alias
#[derive(Debug)]
pub struct AliasedSelectionElement {
    pub(crate) alias: String,
    pub(crate) column: SelectionElement,
}

impl AliasedSelectionElement {
    pub fn new(alias: String, column: SelectionElement) -> Self {
        Self { alias, column }
    }
}

/// The cardinality of a json aggregate
#[derive(Debug)]
pub enum SelectionCardinality {
    One,
    Many,
}

/// A selection of columns in a table
#[derive(Debug)]
pub enum Selection {
    /// A sequence of columns
    Seq(Vec<AliasedSelectionElement>),
    /// A json aggregate. The cardinality determines whether it is a single json object or an array of json objects
    Json(Vec<AliasedSelectionElement>, SelectionCardinality),
}

/// An element that could be selected as a part of a `SELECT <selection-element> <selection-element>` clause.
#[derive(Debug)]
pub enum SelectionElement {
    /// A column in the table
    Physical(ColumnId),
    /// A function such as `SUM(price)`
    Function(Function),
    /// A null literal
    Null,
    /// A json object such as `{"name": "concerts"."name", "price": "concerts"."price"}`
    Object(Vec<(String, SelectionElement)>),
    /// A constant such as `"hello"` (useful to supply it to database and get back the same value). Useful for `__typename` field.
    Constant(String),
    /// A subselect such as `... (SELECT * FROM table)`
    SubSelect(RelationId, Box<AbstractSelect>),
}
