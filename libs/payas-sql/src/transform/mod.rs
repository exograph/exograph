pub(crate) mod pg;
pub(crate) mod transformer;

mod join_util;
mod table_dependency;
mod test_util;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionLevel {
    TopLevel,
    Nested,
}
