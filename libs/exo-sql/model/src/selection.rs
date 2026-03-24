// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Support for selecting columns in a table, including json aggregates

use exo_sql_core::operation::DatabaseExtension;
use exo_sql_core::{ColumnId, RelationId};

use crate::select::AbstractSelect;

/// A selection element along with its alias
#[derive(Debug)]
pub struct AliasedSelectionElement<Ext: DatabaseExtension> {
    pub alias: String,
    pub column: SelectionElement<Ext>,
}

impl<Ext: DatabaseExtension> AliasedSelectionElement<Ext> {
    pub fn new(alias: String, column: SelectionElement<Ext>) -> Self {
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
pub enum Selection<Ext: DatabaseExtension> {
    /// A sequence of columns
    Seq(Vec<AliasedSelectionElement<Ext>>),
    /// A json aggregate. The cardinality determines whether it is a single json object or an array of json objects
    Json(Vec<AliasedSelectionElement<Ext>>, SelectionCardinality),
}

/// An element that could be selected as a part of a `SELECT <selection-element> <selection-element>` clause.
#[derive(Debug)]
pub enum SelectionElement<Ext: DatabaseExtension> {
    /// A column in the table
    Physical(ColumnId),
    /// A function such as `SUM(price)`
    Function(exo_sql_core::operation::Function<Ext>),
    /// A json object such as `{"name": "concerts"."name", "price": "concerts"."price"}`
    Object(Vec<(String, SelectionElement<Ext>)>),
    /// A constant such as `"hello"` (useful to supply it to database and get back the same value). Useful for `__typename` field.
    Constant(String),
    /// A subselect such as `... (SELECT * FROM table)`
    SubSelect(RelationId, Box<AbstractSelect<Ext>>),
}
