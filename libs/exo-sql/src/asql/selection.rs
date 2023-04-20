// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Support for selecting columns in a table, including json aggregates
use maybe_owned::MaybeOwned;

use crate::{sql::physical_column::PhysicalColumn, ColumnPathLink, PhysicalTable};

use super::select::AbstractSelect;

/// A selection element along with its alias
#[derive(Debug)]
pub struct AliasedSelectionElement<'a> {
    pub(crate) alias: String,
    pub(crate) column: SelectionElement<'a>,
}

impl<'a> AliasedSelectionElement<'a> {
    pub fn new(alias: String, column: SelectionElement<'a>) -> Self {
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
pub enum Selection<'a> {
    /// A sequence of columns
    Seq(Vec<AliasedSelectionElement<'a>>),
    /// A json aggregate. The cardinality determines whether it is a single json object or an array of json objects
    Json(Vec<AliasedSelectionElement<'a>>, SelectionCardinality),
}

/// An element that could be selected as a part of a `SELECT <selection-element> <selection-element>` clause.
#[derive(Debug)]
pub enum SelectionElement<'a> {
    /// A column in the table
    Physical(&'a PhysicalColumn),
    /// A function such as `SUM(price)`
    Function {
        function_name: String,
        column: &'a PhysicalColumn,
    },
    /// A json object such as `{"name": "concerts"."name", "price": "concerts"."price"}`
    Object(Vec<(String, MaybeOwned<'a, SelectionElement<'a>>)>),
    /// A constant such as `"hello"` (useful to supply it to database and get back the same value). Useful for `__typename` field.
    Constant(String),
    /// A subselect such as `... FROM (SELECT * FROM table)`
    SubSelect(ColumnPathLink<'a>, AbstractSelect<'a>),
}

/// Relation between two tables
/// The `column` is the column in the one table that is joined to the other `table`('s primary key)
/// TODO: Could this idea be consolidated with the `ColumnPath`? After all, both represent a way to link two tables
#[derive(Debug)]
pub struct NestedElementRelation<'a> {
    pub column: &'a PhysicalColumn,
    pub table: &'a PhysicalTable,
}

impl<'a> NestedElementRelation<'a> {
    pub fn new(column: &'a PhysicalColumn, table: &'a PhysicalTable) -> Self {
        Self { column, table }
    }
}
