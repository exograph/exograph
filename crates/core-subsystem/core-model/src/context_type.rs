//! Contexts are top-level objects that represents the request and environment in a way that can be
//! accessed by queries, mutations and access control expressions. Each context is defined by a
//! [`ContextType`].
//!
//! A typical context is defined as:
//! ```no_rust
//! context AuthContext {
//!   @jwt id: Int
//!   @jwt roles: Array<String>
//! }
//!
//! context IPContext {
//!   @clientId ip: String
//! }
//!
//! context Env {
//!   @env("DEVELOPMENT") isDev: Boolean
//! }
//! ```
//! Annotations like `@jwt` or `@clientId` define how the context is populated from the request and environment.

use serde::{Deserialize, Serialize};

use crate::primitive_type::PrimitiveType;

/// A context type to represent objects such as `AuthContext` and `IPContext`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextType {
    /// Name such as `AuthContext` and `IPContext`
    pub name: String,
    /// Fields such as `@jwt id: Int` and `@clientId ip: String`
    pub fields: Vec<ContextField>,
}

/// A context field is a single field in a context type.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextField {
    /// Name such as `id` and `ip`
    pub name: String,
    /// Type such as `Int` and `String`
    pub typ: ContextFieldType,
    /// Source of the information such as `@jwt` and `@clientId`
    pub source: ContextSource,
}

/// The type of a context field such as `Int` and `Array<String>`
///
/// TODO: We should model this using `FieldType<PrimitiveType>`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ContextFieldType {
    Reference(PrimitiveType),
    Optional(Box<ContextFieldType>),
    List(Box<ContextFieldType>),
}

/// The source of a context field such as JWT or client IP
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextSource {
    /// Name of the source such as `jwt` and `clientIp`
    pub annotation_name: String,
    /// Annotation arguments such as `id` and `roles`
    pub value: Option<String>,
}

impl ContextFieldType {
    pub fn primitive_type(&self) -> &PrimitiveType {
        match self {
            ContextFieldType::Optional(underlying) | ContextFieldType::List(underlying) => {
                underlying.primitive_type()
            }
            ContextFieldType::Reference(pt) => pt,
        }
    }
}
