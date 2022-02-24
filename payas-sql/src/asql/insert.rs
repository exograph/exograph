use maybe_owned::MaybeOwned;

use crate::sql::{
    column::{Column, PhysicalColumn},
    PhysicalTable,
};

#[derive(Debug)]
pub struct AbstractInsert<'a> {
    pub table: &'a PhysicalTable,
    pub column_names: Vec<&'a PhysicalColumn>,
    pub column_values_seq: Vec<Vec<MaybeOwned<'a, Column<'a>>>>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}
