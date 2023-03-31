use super::{
    delete::AbstractDelete, insert::AbstractInsert, select::AbstractSelect, update::AbstractUpdate,
};

/// Top-level abstract operation. A transformed version of this can be submitted to a database.
#[derive(Debug)]
pub enum AbstractOperation<'a> {
    Select(AbstractSelect<'a>),
    Delete(AbstractDelete<'a>),
    Insert(AbstractInsert<'a>),
    Update(AbstractUpdate<'a>),
}
