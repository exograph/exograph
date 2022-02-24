use maybe_owned::MaybeOwned;

use crate::sql::{column::Column, PhysicalTable};

use super::predicate::AbstractPredicate;

#[derive(Debug)]
pub struct Delete<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: Option<AbstractPredicate<'a>>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}
