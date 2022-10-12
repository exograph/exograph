use crate::{
    sql::column::{Column, PhysicalColumn},
    PhysicalTable,
};

use super::{
    delete::AbstractDelete, insert::AbstractInsert, predicate::AbstractPredicate,
    select::AbstractSelect, selection::NestedElementRelation,
};

/// Abstract representation of an update statement.
///
/// An update may have nested create, update, and delete operations. This supports updating a tree of entities
/// starting at the root table. For example, while updating a concert, this allows adding a new concert-artist,
/// updating (say, role or rank) of an existing concert-artist, or deleting an existing concert-artist.
#[derive(Debug)]
pub struct AbstractUpdate<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: Option<AbstractPredicate<'a>>,
    pub column_values: Vec<(&'a PhysicalColumn, Column<'a>)>,
    pub selection: AbstractSelect<'a>,
    pub nested_updates: Vec<NestedAbstractUpdate<'a>>,
    pub nested_inserts: Vec<NestedAbstractInsert<'a>>,
    pub nested_deletes: Vec<NestedAbstractDelete<'a>>,
}

#[derive(Debug)]
pub struct NestedAbstractUpdate<'a> {
    pub relation: NestedElementRelation<'a>,
    pub update: AbstractUpdate<'a>,
}

#[derive(Debug)]
pub struct NestedAbstractInsert<'a> {
    pub relation: NestedElementRelation<'a>,
    pub insert: AbstractInsert<'a>,
}

#[derive(Debug)]
pub struct NestedAbstractDelete<'a> {
    pub relation: NestedElementRelation<'a>,
    pub delete: AbstractDelete<'a>,
}
