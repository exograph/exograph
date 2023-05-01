// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

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

use crate::{mapped_arena::MappedArena, primitive_type::PrimitiveType, types::FieldType};

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
pub type ContextFieldType = FieldType<PrimitiveType>;

/// The source of a context field such as JWT or client IP
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextSource {
    /// Name of the source such as `jwt` and `clientIp`
    pub annotation_name: String,
    /// Annotation arguments such as `id` and `roles`
    pub value: Option<String>,
}

/// Container for all context types. Allows us to abstract over subsystems to share the
/// implementation extracting context (for example, when solving access control expressions)
pub trait ContextContainer {
    /// Get all context types
    ///
    /// This allows us to have a shared implementation of `extract_context`
    fn contexts(&self) -> &MappedArena<ContextType>;
}

/// A path representing context selection such as `AuthContext.role`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextSelection {
    /// The name of the context such as `AuthContext`
    pub context_name: String,
    /// The path to the value within the context such as `role`. Since the path is always non-empty,
    /// it is represented with a tuple of the first element and the rest of the elements.
    pub path: (String, Vec<String>),
}

pub fn get_context<'a>(
    path_elements: &[String],
    contexts: &'a MappedArena<ContextType>,
) -> (ContextSelection, &'a ContextFieldType) {
    if path_elements.len() == 2 {
        let context_type = contexts
            .iter()
            .find(|t| t.1.name == path_elements[0])
            .unwrap()
            .1;
        let field = context_type
            .fields
            .iter()
            .find(|field| field.name == path_elements[1])
            .unwrap();

        (
            ContextSelection {
                context_name: path_elements[0].clone(),
                path: (path_elements[1].clone(), vec![]),
            },
            &field.typ,
        )
    } else {
        todo!() // Nested selection such as AuthContext.user.id
    }
}
