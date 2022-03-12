use maybe_owned::MaybeOwned;

use crate::sql::column::{Column, PhysicalColumn};

#[derive(Debug)]
pub struct ColumnValuePair<'a> {
    pub column: &'a PhysicalColumn,
    pub value: MaybeOwned<'a, Column<'a>>,
}

impl<'a> ColumnValuePair<'a> {
    pub fn new(column: &'a PhysicalColumn, value: MaybeOwned<'a, Column<'a>>) -> Self {
        Self { column, value }
    }
}
