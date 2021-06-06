use payas_model::{
    model::mapped_arena::MappedArena,
    sql::column::{IntBits, PhysicalColumnType},
};
use serde::{Deserialize, Serialize};
use std::ops::Deref;

use super::{TypedAnnotation, TypedField};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Primitive(PrimitiveType),
    Composite(CompositeType),
    Optional(Box<Type>),
    List(Box<Type>),
    Reference(String),
    Defer,
    Error(String),
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
            Type::Error(_) => true,
            Type::Optional(underlying) => underlying.deref().is_error(),
            Type::List(underlying) => underlying.deref().is_error(),
            _ => false,
        }
    }

    pub fn is_incomplete(&self) -> bool {
        self.is_defer() || self.is_error()
    }

    pub fn deref<'a>(&'a self, env: &'a MappedArena<Type>) -> Type {
        match &self {
            Type::Reference(name) => env.get_by_key(name).unwrap().clone(),
            Type::Optional(underlying) => Type::Optional(Box::new(underlying.deref().deref(env))),
            Type::List(underlying) => Type::List(Box::new(underlying.deref().deref(env))),
            o => o.deref().clone(),
        }
    }

    pub fn as_primitive(&self) -> PrimitiveType {
        match &self {
            Type::Primitive(p) => p.clone(),
            _ => panic!("Not a primitive: {:?}", self),
        }
    }

    // useful for relation creation
    pub fn inner_composite<'a>(&'a self, env: &'a MappedArena<Type>) -> &'a CompositeType {
        match &self {
            Type::Composite(c) => c,
            Type::Reference(r) => env.get_by_key(r).unwrap().inner_composite(env),
            Type::Optional(o) => o.inner_composite(env),
            Type::List(o) => o.inner_composite(env),
            _ => panic!("Cannot get inner composite of type {:?}", self),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeType {
    pub name: String,
    pub kind: CompositeTypeKind,
    pub fields: Vec<TypedField>,
    pub annotations: Vec<TypedAnnotation>,
}

impl CompositeType {
    pub fn get_annotation(&self, name: &str) -> Option<&TypedAnnotation> {
        self.annotations.iter().find(|a| a.name == *name)
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
    String,
    Boolean,
}

impl PrimitiveType {
    pub fn to_column_type(&self) -> PhysicalColumnType {
        match &self {
            PrimitiveType::Int => PhysicalColumnType::Int { bits: IntBits::_32 },
            PrimitiveType::String => PhysicalColumnType::String,
            PrimitiveType::Boolean => PhysicalColumnType::Boolean,
        }
    }

    pub fn name(&self) -> &str {
        match &self {
            PrimitiveType::Int => "Int",
            PrimitiveType::String => "String",
            PrimitiveType::Boolean => "Boolean",
        }
    }
}
