// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Common support for representing GraphQL types such as `Int`, `List<Int>`, `Optional<Int>`, `Optional<List<Int>>`, etc.
//!

use async_graphql_parser::types::{BaseType, ConstDirective, Type};
use async_graphql_value::{ConstValue, Name};
use serde::{Deserialize, Serialize};

use crate::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    type_normalization::{default_positioned, default_positioned_name},
};

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
    /// Return the inner type, if any
    pub fn inner(&self) -> Option<&FieldType<T>> {
        match self {
            FieldType::Plain(_) => None,
            FieldType::List(inner) | FieldType::Optional(inner) => Some(inner),
        }
    }

    /// Return the base type (i.e. by removing optional decoration)
    pub fn base_type(&self) -> &FieldType<T> {
        match self {
            FieldType::Optional(inner_typ) => inner_typ.as_ref(),
            _ => self,
        }
    }

    /// Return the innermost (i.e. leaf) type
    pub fn innermost(&self) -> &T {
        match self {
            FieldType::Plain(inner) => inner,
            FieldType::List(inner) | FieldType::Optional(inner) => inner.innermost(),
        }
    }

    /// Wrap the destination type to match the structure of the source type
    pub fn wrap<D>(&self, dest: D) -> FieldType<D> {
        match self {
            FieldType::Plain(_) => FieldType::Plain(dest),
            FieldType::Optional(optional) => FieldType::Optional(Box::new(optional.wrap(dest))),
            FieldType::List(list) => FieldType::List(Box::new(list.wrap(dest))),
        }
    }
}

impl<T: Clone> FieldType<T> {
    /// Compute the optional version of the given type
    pub fn optional(&self) -> Self {
        match self {
            FieldType::Optional(_) => self.clone(), // Already optional
            _ => FieldType::Optional(Box::new(self.clone())),
        }
    }
}

/// Transforms the type into an introspection type
///
/// The complexity of this function is due to the fact that the GraphQL spec and hence the
/// introspection type ([`Type`]) does not support nested optionals. However, [`FieldType`]
/// being more general, does. This function will panic if it encounters a nested optional.
impl<T: Named> From<&FieldType<T>> for Type {
    fn from(ft: &FieldType<T>) -> Self {
        match ft {
            FieldType::Plain(base) => {
                let base_name = base.name();

                if base_name == "Vector" {
                    Type {
                        base: BaseType::List(Box::new(Type {
                            base: BaseType::Named(Name::new("Float")),
                            nullable: false,
                        })),
                        nullable: false,
                    }
                } else {
                    Type {
                        base: BaseType::Named(Name::new(base_name)),
                        nullable: false,
                    }
                }
            }
            FieldType::Optional(underlying) => {
                let Type { base, nullable } = underlying.as_ref().into();

                if nullable {
                    panic!("Optional type cannot be nested")
                }
                Type {
                    base,
                    nullable: true,
                }
            }
            FieldType::List(underlying) => Type {
                base: BaseType::List(Box::new(underlying.as_ref().into())),
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
    pub fn typ_id(&self) -> SerializableSlabIndex<T> {
        match self {
            OperationReturnType::Plain(BaseOperationReturnType {
                associated_type_id, ..
            }) => *associated_type_id,
            OperationReturnType::List(underlying) | OperationReturnType::Optional(underlying) => {
                underlying.typ_id()
            }
        }
    }

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TypeValidation {
    Int { range: (i64, i64) },
    Float { range: (f64, f64) },
}

pub trait TypeValidationProvider {
    fn get_type_validation(&self) -> Option<TypeValidation>;
}

pub trait DirectivesProvider {
    fn get_directives(&self) -> Vec<ConstDirective>;
}

impl DirectivesProvider for TypeValidation {
    fn get_directives(&self) -> Vec<ConstDirective> {
        let mut directives = vec![];
        match self {
            TypeValidation::Int { range } => {
                let (min, max) = range.to_owned();
                directives.push(get_range_directive(min, max));
                directives
            }

            TypeValidation::Float { range } => {
                let (min, max) = range.to_owned();
                directives.push(get_range_directive(min, max));
                directives
            }
        }
    }
}

fn get_range_directive<T: Into<ConstValue>>(min: T, max: T) -> ConstDirective {
    ConstDirective {
        name: default_positioned_name("range"),
        arguments: vec![
            (
                default_positioned_name("min"),
                default_positioned(min.into()),
            ),
            (
                default_positioned_name("max"),
                default_positioned(max.into()),
            ),
        ],
    }
}
