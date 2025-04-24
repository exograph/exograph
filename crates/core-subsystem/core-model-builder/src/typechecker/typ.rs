// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use core_model::primitive_type::PrimitiveType;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, ops::Deref};

use super::Typed;
use crate::ast::ast_types::{AstEnum, AstModel, AstModule};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Primitive(PrimitiveType),
    Composite(AstModel<Typed>),
    Enum(AstEnum<Typed>),
    Optional(Box<Type>),
    Set(Box<Type>),
    Array(Box<Type>),
    Reference(SerializableSlabIndex<Type>),
    Null,
    Defer,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module(pub AstModule<Typed>);

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Primitive(p) => p.fmt(f),
            Type::Composite(c) => c.fmt(f),
            Type::Enum(e) => e.fmt(f),
            Type::Optional(o) => {
                o.fmt(f)?;
                f.write_str("?")
            }
            Type::Set(l) => {
                f.write_str("Set<")?;
                l.fmt(f)?;
                f.write_str(">")
            }
            Type::Array(l) => {
                f.write_str("Array<")?;
                l.fmt(f)?;
                f.write_str(">")
            }
            Type::Reference(r) => {
                f.write_str("ref#")?;
                r.arr_idx().fmt(f)
            }
            Type::Null => f.write_str("null"),
            _ => Result::Err(std::fmt::Error),
        }
    }
}

impl Type {
    pub fn is_defer(&self) -> bool {
        match &self {
            Type::Defer => true,
            Type::Optional(underlying) | Type::Set(underlying) | Type::Array(underlying) => {
                underlying.deref().is_defer()
            }
            _ => false,
        }
    }

    pub fn is_error(&self) -> bool {
        match &self {
            Type::Error => true,
            Type::Optional(underlying) | Type::Set(underlying) | Type::Array(underlying) => {
                underlying.deref().is_error()
            }
            _ => false,
        }
    }

    pub fn is_incomplete(&self) -> bool {
        self.is_defer() || self.is_error()
    }

    pub fn is_complete(&self) -> bool {
        !self.is_incomplete()
    }

    pub fn get_underlying_typename(&self, types: &MappedArena<Type>) -> Option<String> {
        match &self {
            Type::Composite(c) => Some(c.name.clone()),
            Type::Enum(e) => Some(e.name.clone()),
            Type::Reference(_id) => self.deref(types).get_underlying_typename(types),
            Type::Primitive(pt) => Some(pt.name()),
            Type::Optional(underlying) | Type::Set(underlying) | Type::Array(underlying) => {
                underlying.get_underlying_typename(types)
            }
            _ => None,
        }
    }

    pub fn deref<'a>(&'a self, types: &'a MappedArena<Type>) -> Type {
        match self {
            Type::Reference(idx) => types[*idx].clone(),
            Type::Optional(underlying) => {
                Type::Optional(Box::new(underlying.as_ref().deref(types)))
            }
            Type::Set(underlying) => Type::Set(Box::new(underlying.as_ref().deref(types))),
            Type::Array(underlying) => Type::Array(Box::new(underlying.as_ref().deref(types))),
            o => o.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypecheckedSystem {
    pub types: MappedArena<Type>,
    pub modules: MappedArena<Module>,
}
