use async_graphql_parser::types::{BaseType, Type};
use async_graphql_value::Name;
use serde::{Deserialize, Serialize};

use crate::mapped_arena::{SerializableSlab, SerializableSlabIndex};

/// A type that can be used as a type for fields and return types
/// Currently supports only list and optional decorations
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FieldType<T> {
    Plain(T),
    List(Box<FieldType<T>>),
    Optional(Box<FieldType<T>>),
}

pub trait Named {
    fn name(&self) -> &str;
}

impl<T> FieldType<T> {
    pub fn inner(&self) -> Option<&FieldType<T>> {
        match self {
            FieldType::Plain(_) => None,
            FieldType::List(inner) | FieldType::Optional(inner) => Some(inner),
        }
    }

    pub fn inner_most(&self) -> &T {
        match self {
            FieldType::Plain(inner) => inner,
            FieldType::List(inner) | FieldType::Optional(inner) => inner.inner_most(),
        }
    }
}

impl<T: Clone> FieldType<T> {
    /// Compute the optional version of the given type
    pub fn optional(&self) -> Self {
        match self {
            FieldType::Optional(_) => self.clone(),
            _ => FieldType::Optional(Box::new(self.clone())),
        }
    }
}

impl<T: Named> FieldType<T> {
    /// Transforms the type into an introspection type
    ///
    /// The complexity of this function is due to the fact that the GraphQL spec and hence the
    /// introspection type (`Type`) does not support nested optionals. However, `FieldType`
    /// being more general, does. This function will panic if it encounters a nested optional.
    pub fn to_introspection_type(&self) -> Type {
        match self {
            FieldType::Plain(base) => Type {
                base: BaseType::Named(Name::new(base.name())),
                nullable: false,
            },
            FieldType::Optional(underlying) => {
                let Type { base, nullable } = underlying.to_introspection_type();

                if nullable {
                    panic!("Optional type cannot be nested")
                }
                Type {
                    base,
                    nullable: true,
                }
            }
            FieldType::List(underlying) => Type {
                base: BaseType::List(Box::new(underlying.to_introspection_type())),
                nullable: false,
            },
        }
    }
}

impl<T: Named> Named for FieldType<T> {
    fn name(&self) -> &str {
        match self {
            FieldType::Plain(plain) => plain.name(),
            FieldType::List(list) => list.name(),
            FieldType::Optional(optional) => optional.name(),
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

pub type OperationReturnType<T> = FieldType<BaseOperationReturnType<T>>;

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
