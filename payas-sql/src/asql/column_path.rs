use maybe_owned::MaybeOwned;

use crate::sql::{column::PhysicalColumn, predicate::LiteralEquality, PhysicalTable, SQLParam};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ColumnPathLink<'a> {
    pub self_column: (&'a PhysicalColumn, &'a PhysicalTable), // We need to keep the table since column carries the table name and not the table itself
    pub linked_column: Option<(&'a PhysicalColumn, &'a PhysicalTable)>,
}

#[derive(Debug, PartialEq)]
pub enum ColumnPath<'a> {
    Physical(Vec<ColumnPathLink<'a>>),
    Literal(MaybeOwned<'a, Box<dyn SQLParam>>),
    Null,
}

impl LiteralEquality for ColumnPath<'_> {
    fn literal_eq(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Self::Literal(v1), Self::Literal(v2)) => Some(v1 == v2),
            _ => None,
        }
    }
}
