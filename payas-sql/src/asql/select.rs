use crate::{Limit, Offset, PhysicalTable};

use super::{order_by::AbstractOrderBy, predicate::AbstractPredicate, selection::Selection};

#[derive(Debug)]
pub struct AbstractSelect<'a> {
    pub table: &'a PhysicalTable,
    pub selection: Selection<'a>,
    pub predicate: Option<AbstractPredicate<'a>>,
    pub order_by: Option<AbstractOrderBy<'a>>,
    pub offset: Option<Offset>,
    pub limit: Option<Limit>,
}

pub enum SelectionLevel {
    TopLevel,
    Nested,
}
