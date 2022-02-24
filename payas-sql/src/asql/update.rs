use maybe_owned::MaybeOwned;

use crate::sql::{
    column::{Column, PhysicalColumn, ProxyColumn},
    PhysicalTable,
};

use super::predicate::AbstractPredicate;

#[derive(Debug)]
struct AbstractUpdate<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: Option<AbstractPredicate<'a>>,
    pub column_values: Vec<(&'a PhysicalColumn, ProxyColumn<'a>)>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}
