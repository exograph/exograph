use maybe_owned::MaybeOwned;

use crate::sql::{column::PhysicalColumn, PhysicalTable, SQLParam};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ColumnPathLink<'a> {
    pub self_column: (&'a PhysicalColumn, &'a PhysicalTable), // We need to keep the table since column carries the table name and not the table itself
    pub linked_column: Option<(&'a PhysicalColumn, &'a PhysicalTable)>,
}

#[derive(Debug, PartialEq)]
pub enum ColumnPath<'a> {
    Physical(Vec<ColumnPathLink<'a>>),
    Literal(MaybeOwned<'a, Box<dyn SQLParam>>),
}

impl<'a> ColumnPath<'a> {
    pub fn leaf_column(&self) -> &'a PhysicalColumn {
        match self {
            ColumnPath::Physical(links) => links.last().unwrap().self_column.0,
            ColumnPath::Literal(_) => panic!("Cannot get leaf column from literal"),
        }
    }

    pub fn from_column(column: &'a PhysicalColumn, table: &'a PhysicalTable) -> Self {
        ColumnPath::Physical(vec![ColumnPathLink {
            self_column: (column, table),
            linked_column: None,
        }])
    }

    pub fn from_column_path_and_column(
        column: &'a PhysicalColumn,
        table: &'a PhysicalTable,
    ) -> Self {
        ColumnPath::Physical(vec![ColumnPathLink {
            self_column: (column, table),
            linked_column: None,
        }])
    }
}
