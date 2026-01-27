// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! RPC Schema types that represent the structure of RPC methods and their parameters.
//!
//! These types are used both for introspection (generating OpenRPC documents) and
//! for validation (checking incoming parameters against the schema).

use core_model::types::TypeValidation;
use serde::{Deserialize, Serialize};

/// The complete RPC schema containing all methods and component definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcSchema {
    /// All available RPC methods
    pub methods: Vec<RpcMethod>,
    /// Reusable schema components (object types, etc.)
    pub components: RpcComponents,
}

impl RpcSchema {
    pub fn new() -> Self {
        Self {
            methods: Vec::new(),
            components: RpcComponents::new(),
        }
    }

    pub fn add_method(&mut self, method: RpcMethod) {
        self.methods.push(method);
    }

    pub fn add_object_type(&mut self, name: String, object_type: RpcObjectType) {
        self.components.schemas.push((name, object_type));
    }

    /// Merge another schema into this one, consuming the other schema.
    pub fn merge(&mut self, other: RpcSchema) {
        self.methods.extend(other.methods);
        self.components.schemas.extend(other.components.schemas);
    }
}

impl Default for RpcSchema {
    fn default() -> Self {
        Self::new()
    }
}

/// Reusable schema components (similar to JSON Schema's $defs or OpenAPI's components).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RpcComponents {
    /// Object type definitions that can be referenced by methods
    pub schemas: Vec<(String, RpcObjectType)>,
}

impl RpcComponents {
    pub fn new() -> Self {
        Self {
            schemas: Vec::new(),
        }
    }

    pub fn get_schema(&self, name: &str) -> Option<&RpcObjectType> {
        self.schemas.iter().find(|(n, _)| n == name).map(|(_, s)| s)
    }
}

/// An RPC method definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcMethod {
    /// The method name (e.g., "get_todos", "get_todo")
    pub name: String,
    /// Optional description of what the method does
    pub description: Option<String>,
    /// Parameters for this method
    pub params: Vec<RpcParameter>,
    /// The result type schema
    pub result: RpcTypeSchema,
}

impl RpcMethod {
    pub fn new(name: String, result: RpcTypeSchema) -> Self {
        Self {
            name,
            description: None,
            params: Vec::new(),
            result,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_param(mut self, param: RpcParameter) -> Self {
        self.params.push(param);
        self
    }
}

/// A parameter for an RPC method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcParameter {
    /// The parameter name
    pub name: String,
    /// The parameter's type schema.
    /// If the schema is `RpcTypeSchema::Optional`, the parameter is not required.
    pub schema: RpcTypeSchema,
    /// Optional description of the parameter
    pub description: Option<String>,
}

impl RpcParameter {
    pub fn new(name: impl Into<String>, schema: RpcTypeSchema) -> Self {
        Self {
            name: name.into(),
            schema,
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Returns true if this parameter is required (i.e., not optional)
    pub fn is_required(&self) -> bool {
        !matches!(self.schema, RpcTypeSchema::Optional { .. })
    }
}

/// Schema for an RPC type, supporting primitives, objects, arrays, and optionals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcTypeSchema {
    /// A scalar/primitive type with optional validation constraints
    Scalar {
        type_name: String,
        validation: Option<TypeValidation>,
    },
    /// An enumeration type
    Enum { values: Vec<String> },
    /// A reference to an object type defined in components
    Object { type_ref: String },
    /// An array of items
    Array { items: Box<RpcTypeSchema> },
    /// An optional type (wraps another type)
    Optional { inner: Box<RpcTypeSchema> },
}

impl RpcTypeSchema {
    /// Create a scalar type schema
    pub fn scalar(type_name: impl Into<String>) -> Self {
        Self::Scalar {
            type_name: type_name.into(),
            validation: None,
        }
    }

    /// Create a scalar type with validation
    pub fn scalar_with_validation(
        type_name: impl Into<String>,
        validation: TypeValidation,
    ) -> Self {
        Self::Scalar {
            type_name: type_name.into(),
            validation: Some(validation),
        }
    }

    /// Create an enum type schema
    pub fn enum_type(values: Vec<String>) -> Self {
        Self::Enum { values }
    }

    /// Create an object reference schema
    pub fn object(type_ref: impl Into<String>) -> Self {
        Self::Object {
            type_ref: type_ref.into(),
        }
    }

    /// Create an array type schema
    pub fn array(items: RpcTypeSchema) -> Self {
        Self::Array {
            items: Box::new(items),
        }
    }

    /// Create an optional type schema
    pub fn optional(inner: RpcTypeSchema) -> Self {
        Self::Optional {
            inner: Box::new(inner),
        }
    }

    /// Wrap this schema to make it optional (if not already)
    pub fn into_optional(self) -> Self {
        match self {
            Self::Optional { .. } => self, // Already optional
            _ => Self::Optional {
                inner: Box::new(self),
            },
        }
    }

    /// Get the inner type name for scalar types
    pub fn type_name(&self) -> Option<&str> {
        match self {
            Self::Scalar { type_name, .. } => Some(type_name),
            Self::Object { type_ref, .. } => Some(type_ref),
            Self::Optional { inner, .. } => inner.type_name(),
            Self::Array { items, .. } => items.type_name(),
            Self::Enum { .. } => None,
        }
    }
}

/// An object type definition with named fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcObjectType {
    /// The object type name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// The fields of this object
    pub fields: Vec<RpcObjectField>,
}

impl RpcObjectType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            fields: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_field(mut self, field: RpcObjectField) -> Self {
        self.fields.push(field);
        self
    }
}

/// A field within an object type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcObjectField {
    /// The field name
    pub name: String,
    /// The field's type schema
    pub schema: RpcTypeSchema,
    /// Optional description
    pub description: Option<String>,
}

impl RpcObjectField {
    pub fn new(name: impl Into<String>, schema: RpcTypeSchema) -> Self {
        Self {
            name: name.into(),
            schema,
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::types::IntConstraints;

    #[test]
    fn test_rpc_schema_builder() {
        let mut schema = RpcSchema::new();

        // Add an object type
        let todo_type = RpcObjectType::new("Todo")
            .with_description("A todo item")
            .with_field(RpcObjectField::new("id", RpcTypeSchema::scalar("Int")))
            .with_field(RpcObjectField::new(
                "title",
                RpcTypeSchema::scalar("String"),
            ))
            .with_field(RpcObjectField::new(
                "priority",
                RpcTypeSchema::scalar_with_validation(
                    "Int",
                    TypeValidation::Int(IntConstraints::from_range(1, 5)),
                ),
            ));
        schema.add_object_type("Todo".to_string(), todo_type);

        // Add a method
        let method = RpcMethod::new(
            "get_todos".to_string(),
            RpcTypeSchema::array(RpcTypeSchema::object("Todo")),
        )
        .with_description("Get all todos")
        .with_param(RpcParameter::new(
            "where",
            RpcTypeSchema::optional(RpcTypeSchema::object("TodoFilter")),
        ));
        schema.add_method(method);

        assert_eq!(schema.methods.len(), 1);
        assert_eq!(schema.components.schemas.len(), 1);
    }

    #[test]
    fn test_parameter_required() {
        let required_param = RpcParameter::new("id", RpcTypeSchema::scalar("Int"));
        assert!(required_param.is_required());

        let optional_param = RpcParameter::new(
            "filter",
            RpcTypeSchema::optional(RpcTypeSchema::object("Filter")),
        );
        assert!(!optional_param.is_required());
    }
}
