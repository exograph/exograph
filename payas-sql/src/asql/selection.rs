use crate::sql::column::PhysicalColumn;

use super::select::AbstractSelect;

pub enum Selection<'a> {
    Physical(&'a PhysicalColumn),
    Compound(AbstractSelect<'a>),
}
