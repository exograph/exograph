use crate::sql::{column::PhysicalColumn, PhysicalTable, SQLParam};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ColumnPathLink<'a> {
    pub self_column: (&'a PhysicalColumn, &'a PhysicalTable), // We need to keep the table since column carries the table name and not the table itself
    pub linked_column: Option<(&'a PhysicalColumn, &'a PhysicalTable)>,
}

#[derive(Debug)]
pub enum ColumnPath<'a> {
    Physical(Vec<ColumnPathLink<'a>>),
    Literal(Box<dyn SQLParam>),
}

impl<'a> ColumnPath<'a> {
    pub fn leaf_column(&self) -> &PhysicalColumn {
        match self {
            ColumnPath::Physical(links) => links.last().unwrap().self_column.0,
            ColumnPath::Literal(_) => panic!("Cannot get leaf column from literal"),
        }
    }
}
