use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    ops::Deref,
};

use super::{AnnotationMap, TypedField};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Primitive(PrimitiveType),
    Composite(CompositeType),
    Optional(Box<Type>),
    List(Box<Type>),
    Reference(String),
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
            Type::List(l) => {
                f.write_str("[")?;
                l.fmt(f)?;
                f.write_str("]")
            }
            Type::Reference(r) => f.write_str(r.as_str()),
            _ => Result::Err(std::fmt::Error),
        }
    }
}

impl Type {
    pub fn is_defer(&self) -> bool {
        match &self {
            Type::Defer => true,
            Type::Optional(underlying) => underlying.deref().is_defer(),
            Type::List(underlying) => underlying.deref().is_defer(),
            _ => false,
        }
    }

    pub fn is_error(&self) -> bool {
        match &self {
            Type::Error => true,
            Type::Optional(underlying) => underlying.deref().is_error(),
            Type::List(underlying) => underlying.deref().is_error(),
            _ => false,
        }
    }

    pub fn is_incomplete(&self) -> bool {
        self.is_defer() || self.is_error()
    }

    pub fn get_underlying_typename(&self) -> Option<String> {
        match &self {
            Type::Reference(name) => Some(name.to_owned()),
            Type::Primitive(pt) => Some(pt.name()),
            Type::Optional(underlying) => underlying.get_underlying_typename(),
            Type::List(underlying) => underlying.get_underlying_typename(),
            _ => None,
        }
    }

    pub fn deref<'a>(&'a self, env: &'a MappedArena<Type>) -> Type {
        match &self {
            Type::Reference(name) => env.get_by_key(name).unwrap().clone(),
            Type::Optional(underlying) => Type::Optional(Box::new(underlying.deref().deref(env))),
            Type::List(underlying) => Type::List(Box::new(underlying.deref().deref(env))),
            o => o.deref().clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeType {
    pub name: String,
    pub kind: CompositeTypeKind,
    pub fields: Vec<TypedField>,
    pub annotations: Box<AnnotationMap>,
}

impl Display for CompositeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CompositeTypeKind {
    Persistent,
    Context,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveType {
    Int,
    Float,
    String,
    Boolean,
    LocalDate,
    LocalTime,
    LocalDateTime,
    Instant,
    Json,
    Array(Box<PrimitiveType>),
}

impl PrimitiveType {
    pub fn name(&self) -> String {
        if let PrimitiveType::Array(pt) = &self {
            return format!("[{}]", pt.name());
        }

        match &self {
            PrimitiveType::Int => "Int",
            PrimitiveType::Float => "Float",
            PrimitiveType::String => "String",
            PrimitiveType::Boolean => "Boolean",
            PrimitiveType::LocalDate => "LocalDate",
            PrimitiveType::LocalTime => "LocalTime",
            PrimitiveType::LocalDateTime => "LocalDateTime",
            PrimitiveType::Instant => "Instant",
            PrimitiveType::Json => "Json",
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
