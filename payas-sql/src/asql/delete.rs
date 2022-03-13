use crate::PhysicalTable;

use super::{predicate::AbstractPredicate, select::AbstractSelect};

#[derive(Debug)]
pub struct AbstractDelete<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: Option<AbstractPredicate<'a>>,
    pub selection: AbstractSelect<'a>,
}
