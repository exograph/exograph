use serde::{Deserialize, Serialize};

use crate::mapped_arena::{SerializableSlab, SerializableSlabIndex};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BaseOperationReturnType<T> {
    pub associated_type_id: SerializableSlabIndex<T>,
    pub type_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OperationReturnType<T> {
    Plain(BaseOperationReturnType<T>),
    List(Box<OperationReturnType<T>>),
    Optional(Box<OperationReturnType<T>>),
}

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
