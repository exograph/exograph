use crate::{
    sql::column::{Column, PhysicalColumn},
    PhysicalTable,
};

use super::select::AbstractSelect;

#[derive(Debug)]
pub struct ColumnSelection<'a> {
    pub(crate) alias: String,
    pub(crate) column: SelectionElement<'a>,
}

impl<'a> ColumnSelection<'a> {
    pub fn new(alias: String, column: SelectionElement<'a>) -> Self {
        Self { alias, column }
    }
}

#[derive(Debug)]
pub enum SelectionCardinality {
    One,
    Many,
}

#[derive(Debug)]
pub enum Selection<'a> {
    Seq(Vec<ColumnSelection<'a>>),
    Json(Vec<ColumnSelection<'a>>, SelectionCardinality),
}

pub enum SelectionSQL<'a> {
    Single(Column<'a>),
    Seq(Vec<Column<'a>>),
}

#[derive(Debug)]
pub enum SelectionElement<'a> {
    Physical(&'a PhysicalColumn),
    Constant(String), // To support __typename
    Nested(NestedElementRelation<'a>, AbstractSelect<'a>),
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
