use async_graphql_parser::types::{BaseType, Type};
use async_graphql_value::Name;
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

impl<T> DecoratedType<T> {
    pub fn inner(&self) -> Option<&DecoratedType<T>> {
        match self {
            DecoratedType::Plain(_) => None,
            DecoratedType::List(inner) | DecoratedType::Optional(inner) => Some(inner),
        }
    }

    pub fn inner_most(&self) -> &T {
        match self {
            DecoratedType::Plain(inner) => inner,
            DecoratedType::List(inner) | DecoratedType::Optional(inner) => inner.inner_most(),
        }
    }
}

impl<T: Named> DecoratedType<T> {
    /// Transforms the type into an introspection type
    ///
    /// The complexity of this function is due to the fact that the GraphQL spec and hence the
    /// introspection type (`Type`) does not support nested optionals. However, `DecoratedType`
    /// being more general, does support nested optionals. This function will panic if it encounters
    /// a nested optional.
    pub fn to_introspection_type(&self) -> Type {
        /// Returns the base type and whether it is optional
        fn base_type<T: Named>(typ: &DecoratedType<T>) -> (BaseType, bool) {
            match typ {
                DecoratedType::Plain(base) => (BaseType::Named(Name::new(base.name())), false),
                DecoratedType::List(underlying) => (
                    BaseType::List(Box::new(underlying.to_introspection_type())),
                    false,
                ),
                DecoratedType::Optional(underlying) => (base_type(underlying).0, true),
            }
        }

        match self {
            DecoratedType::Plain(_) => Type {
                base: base_type(self).0,
                nullable: false,
            },
            DecoratedType::Optional(underlying) => {
                let (base, is_optional) = base_type(underlying);

                if is_optional {
                    panic!("Optional type cannot be nested")
                }
                Type {
                    base,
                    nullable: true,
                }
            }
            DecoratedType::List(underlying) => Type {
                base: base_type(underlying).0,
                nullable: false,
            },
        }
    }
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
