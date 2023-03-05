use std::cmp::Ordering;

use crate::{
    sql::{column::PhysicalColumn, predicate::LiteralEquality, SQLParamContainer},
    PhysicalTable,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ColumnPathLink<'a> {
    pub self_column: (&'a PhysicalColumn, &'a PhysicalTable), // We need to keep the table since a column carries the table name and not the table itself
    pub linked_column: Option<(&'a PhysicalColumn, &'a PhysicalTable)>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ColumnPath<'a> {
    Physical(Vec<ColumnPathLink<'a>>),
    Literal(SQLParamContainer),
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
                &link.self_column.0.column_name,
                &link.self_column.1.name,
                link.linked_column.map(|ref c| c.0.column_name.as_str()),
                link.linked_column.map(|ref c| c.1.name.as_str()),
            )
        }

        tupled(self).cmp(&tupled(other))
    }
}
