use crate::PhysicalTable;

use super::{predicate::AbstractPredicate, select::AbstractSelect};

/// Abstract representation of a delete operation
#[derive(Debug)]
pub struct AbstractDelete<'a> {
    /// The table to delete from
    pub table: &'a PhysicalTable,
    /// The predicate to filter rows.
    pub predicate: AbstractPredicate<'a>,
    /// The selection to return
    pub selection: AbstractSelect<'a>,
}
