use serde::{Deserialize, Serialize};

use crate::mapped_arena::{SerializableSlab, SerializableSlabIndex};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DecoratedType<T> {
    Plain(T),
    List(Box<DecoratedType<T>>),
    Optional(Box<DecoratedType<T>>),
}

pub trait Named {
    fn name(&self) -> &str;
}

impl<T: Named> Named for DecoratedType<T> {
    fn name(&self) -> &str {
        match self {
            DecoratedType::Plain(plain) => plain.name(),
            DecoratedType::List(list) => list.name(),
            DecoratedType::Optional(optional) => optional.name(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BaseOperationReturnType<T> {
    pub associated_type_id: SerializableSlabIndex<T>,
    pub type_name: String,
}

impl<T> Named for BaseOperationReturnType<T> {
    fn name(&self) -> &str {
        &self.type_name
    }
}

pub type OperationReturnType<T> = DecoratedType<BaseOperationReturnType<T>>;

impl<T> OperationReturnType<T> {
    pub fn typ<'a>(&'a self, types: &'a SerializableSlab<T>) -> &T {
        match self {
            OperationReturnType::Plain(BaseOperationReturnType {
                associated_type_id, ..
            }) => &types[*associated_type_id],
            OperationReturnType::List(underlying) | OperationReturnType::Optional(underlying) => {
                underlying.typ(types)
            }
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            OperationReturnType::Plain(BaseOperationReturnType { type_name, .. }) => type_name,
            OperationReturnType::List(underlying) | OperationReturnType::Optional(underlying) => {
                underlying.type_name()
            }
        }
    }
}
