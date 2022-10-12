use super::{
    delete::AbstractDelete, insert::AbstractInsert, select::AbstractSelect, update::AbstractUpdate,
};

#[derive(Debug)]
pub enum AbstractOperation<'a> {
    Select(AbstractSelect<'a>),
    Delete(AbstractDelete<'a>),
    Insert(AbstractInsert<'a>),
    Update(AbstractUpdate<'a>),
}
