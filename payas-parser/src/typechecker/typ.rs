use payas_model::model::mapped_arena::{MappedArena, SerializableSlabIndex};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    ops::Deref,
};

use super::Typed;
use crate::ast::ast_types::{AstModel, AstService};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Primitive(PrimitiveType),
    Composite(AstModel<Typed>),
    Service(AstService<Typed>),
    Optional(Box<Type>),
    Set(Box<Type>),
    Array(Box<Type>),
    Reference(SerializableSlabIndex<Type>),
    Defer,
    Error,
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Primitive(p) => p.fmt(f),
            Type::Composite(c) => c.fmt(f),
            Type::Optional(o) => {
                o.fmt(f)?;
                f.write_str("?")
            }
            Type::Set(l) => {
                f.write_str("Set[")?;
                l.fmt(f)?;
                f.write_str("]")
            }
            Type::Array(l) => {
                f.write_str("Array[")?;
                l.fmt(f)?;
                f.write_str("]")
            }
            Type::Reference(r) => {
                f.write_str("ref#")?;
                r.arr_idx().fmt(f)
            }
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

    pub fn get_underlying_typename(&self, types: &MappedArena<Type>) -> Option<String> {
        match &self {
            Type::Composite(c) => Some(c.name.clone()),
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveType {
    Int,
    Float,
    Decimal,
    String,
    Boolean,
    LocalDate,
    LocalTime,
    LocalDateTime,
    Instant,
    Json,
    Array(Box<PrimitiveType>),
    ClaytipInjected,
    Interception(String), // Types such as "Operation" that an interceptor is passed to
}

impl PrimitiveType {
    pub fn name(&self) -> String {
        if let PrimitiveType::Array(pt) = &self {
            return format!("[{}]", pt.name());
        }

        match &self {
            PrimitiveType::Int => "Int",
            PrimitiveType::Float => "Float",
            PrimitiveType::Decimal => "Decimal",
            PrimitiveType::String => "String",
            PrimitiveType::Boolean => "Boolean",
            PrimitiveType::LocalDate => "LocalDate",
            PrimitiveType::LocalTime => "LocalTime",
            PrimitiveType::LocalDateTime => "LocalDateTime",
            PrimitiveType::Instant => "Instant",
            PrimitiveType::Json => "Json",
            PrimitiveType::ClaytipInjected => "ClaytipInjected",
            PrimitiveType::Interception(name) => name,
            PrimitiveType::Array(_) => panic!(),
        }
        .to_owned()
    }
}

impl Display for PrimitiveType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name())
    }
}
