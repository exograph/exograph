use std::cmp::Ordering;

use maybe_owned::MaybeOwned;

use crate::sql::{column::PhysicalColumn, predicate::LiteralEquality, PhysicalTable, SQLParam};

#[derive(Debug, PartialEq, Eq, Clone)]
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

impl<'a> PartialOrd for ColumnPathLink<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for ColumnPathLink<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        fn tupled<'a>(
            link: &'a ColumnPathLink,
        ) -> (&'a str, &'a str, Option<&'a str>, Option<&'a str>) {
            (
                &link.self_column.0.table_name,
                &link.self_column.1.name,
                link.linked_column.map(|ref c| c.0.table_name.as_str()),
                link.linked_column.map(|ref c| c.1.name.as_str()),
            )
        }

        tupled(self).cmp(&tupled(other))
    }
}
