use super::{
    delete::AbstractDelete, insert::AbstractInsert, select::AbstractSelect, update::AbstractUpdate,
};

pub enum AbstractOperation<'a> {
    Select(AbstractSelect<'a>),
    Delete(AbstractDelete<'a>),
    Insert(AbstractInsert<'a>),
    Update(AbstractUpdate<'a>),
}
