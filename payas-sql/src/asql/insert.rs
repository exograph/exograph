use maybe_owned::MaybeOwned;

use crate::sql::{
    column::{Column, PhysicalColumn},
    transaction::TransactionScript,
    PhysicalTable,
};

use super::{select::AbstractSelect, selection::NestedElementRelation};

#[derive(Debug)]
pub struct AbstractInsert<'a> {
    pub table: &'a PhysicalTable,
    pub column_names: Vec<&'a PhysicalColumn>,
    pub column_values_seq: Vec<Vec<MaybeOwned<'a, Column<'a>>>>,
    pub selection: AbstractSelect<'a>,
    pub nested_create: Option<Vec<NestedAbstractInsert<'a>>>,
}

#[derive(Debug)]
pub struct NestedAbstractInsert<'a> {
    pub relation: NestedElementRelation<'a>,
    pub update: AbstractInsert<'a>,
}

// impl<'a> AbstractInsert<'a> {
//     pub fn to_sql(self) -> TransactionScript<'a> {}
// }
