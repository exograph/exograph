use crate::sql::PhysicalTable;

use super::{common::ColumnValuePair, select::AbstractSelect, selection::NestedElementRelation};

#[derive(Debug)]
pub struct NestedInsertion<'a> {
    pub relation: NestedElementRelation<'a>,
    pub self_table: &'a PhysicalTable,
    pub parent_table: &'a PhysicalTable,
    pub insertions: Vec<InsertionRow<'a>>,
}

/// Logical element to be inserted. Each element could be thought of as an
/// attribute of the logical document. For example, with Venue <-> [Concert]
/// model, logical elements in in `Venue` includes its own columns (name,
/// address, etc.), which would be represented as the `SelfInsert` variant. It
/// also includes the logically nested "concerts" element, which would be
/// represented as the `NestedInsert` variant.
#[derive(Debug)]
pub enum InsertionElement<'a> {
    SelfInsert(ColumnValuePair<'a>),
    NestedInsert(NestedInsertion<'a>),
}

/// A logical row to be inserted (see `InsertionElement` for more details).
#[derive(Debug)]
pub struct InsertionRow<'a> {
    pub elems: Vec<InsertionElement<'a>>,
}

#[derive(Debug)]
pub struct AbstractInsert<'a> {
    pub table: &'a PhysicalTable,
    pub rows: Vec<InsertionRow<'a>>,
    pub selection: AbstractSelect<'a>,
}

impl<'a> InsertionRow<'a> {
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
