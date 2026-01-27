// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! OpenRPC specification types.
//!
//! This module defines types that conform to the OpenRPC 1.3.2 specification.
//! See: https://spec.open-rpc.org/

use serde::{Deserialize, Serialize};

/// The root OpenRPC document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRpcDocument {
    /// The OpenRPC specification version (e.g., "1.3.2")
    pub openrpc: String,
    /// Metadata about the API
    pub info: InfoObject,
    /// The available RPC methods
    pub methods: Vec<MethodObject>,
    /// Reusable schema components
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Components>,
}

impl OpenRpcDocument {
    /// Create a new OpenRPC document with the given title and version
    pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            openrpc: "1.3.2".to_string(),
            info: InfoObject {
                title: title.into(),
                version: version.into(),
                description: None,
            },
            methods: Vec::new(),
            components: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.info.description = Some(description.into());
        self
    }

    pub fn with_method(mut self, method: MethodObject) -> Self {
        self.methods.push(method);
        self
    }

    pub fn with_components(mut self, components: Components) -> Self {
        self.components = Some(components);
        self
    }
}

/// Metadata about the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoObject {
    /// The title of the API
    pub title: String,
    /// The version of the API (not the OpenRPC spec version)
    pub version: String,
    /// A description of the API
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// An RPC method definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodObject {
    /// The canonical name of the method
    pub name: String,
    /// A description of what the method does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// A summary of the method
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// The parameters for this method
    pub params: Vec<ContentDescriptor>,
    /// The result of calling this method
    pub result: ContentDescriptor,
}

impl MethodObject {
    pub fn new(name: impl Into<String>, result: ContentDescriptor) -> Self {
        Self {
            name: name.into(),
            description: None,
            summary: None,
            params: Vec::new(),
            result,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    pub fn with_param(mut self, param: ContentDescriptor) -> Self {
        self.params.push(param);
        self
    }
}

/// Describes a method parameter or result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentDescriptor {
    /// The name of the content
    pub name: String,
    /// A description of the content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this parameter is required (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// The JSON Schema describing the content
    pub schema: JsonSchema,
}

impl ContentDescriptor {
    pub fn new(name: impl Into<String>, schema: JsonSchema) -> Self {
        Self {
            name: name.into(),
            description: None,
            required: None,
            schema,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn required(mut self) -> Self {
        self.required = Some(true);
        self
    }

    pub fn optional(mut self) -> Self {
        self.required = Some(false);
        self
    }
}

/// Reusable schema components.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Components {
    /// Schema definitions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schemas: Option<serde_json::Map<String, serde_json::Value>>,
}

impl Components {
    pub fn new() -> Self {
        Self { schemas: None }
    }

    pub fn with_schema(mut self, name: impl Into<String>, schema: JsonSchema) -> Self {
        let schemas = self.schemas.get_or_insert_with(serde_json::Map::new);
        schemas.insert(
            name.into(),
            serde_json::to_value(schema).expect("Failed to serialize schema"),
        );
        self
    }
}

/// A JSON Schema definition.
///
/// This is a simplified representation that covers the types we need.
/// For full JSON Schema support, consider using a dedicated crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum JsonSchema {
    /// A reference to another schema
    Ref(JsonSchemaRef),
    /// An inline schema definition
    Inline(JsonSchemaInline),
}

/// A reference to another schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchemaRef {
    #[serde(rename = "$ref")]
    pub ref_path: String,
}

impl JsonSchemaRef {
    pub fn new(ref_path: impl Into<String>) -> Self {
        Self {
            ref_path: ref_path.into(),
        }
    }

    /// Create a reference to a schema in the components section
    pub fn component(name: impl Into<String>) -> Self {
        Self {
            ref_path: format!("#/components/schemas/{}", name.into()),
        }
    }
}

/// An inline JSON Schema definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JsonSchemaInline {
    /// The type of the schema
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub schema_type: Option<String>,

    /// For string types, allowed enum values
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,

    /// For array types, the schema of items
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<JsonSchema>>,

    /// For object types, property schemas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,

    /// Required properties for objects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,

    /// Whether this can be null
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nullable: Option<bool>,

    /// Description of the schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // Numeric constraints
    /// Minimum value (inclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<serde_json::Number>,

    /// Maximum value (inclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<serde_json::Number>,

    // String constraints
    /// Minimum length for strings
    #[serde(rename = "minLength", skip_serializing_if = "Option::is_none")]
    pub min_length: Option<usize>,

    /// Maximum length for strings
    #[serde(rename = "maxLength", skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
}

impl JsonSchemaInline {
    pub fn new(schema_type: impl Into<String>) -> Self {
        Self {
            schema_type: Some(schema_type.into()),
            ..Default::default()
        }
    }

    pub fn integer() -> Self {
        Self::new("integer")
    }

    pub fn number() -> Self {
        Self::new("number")
    }

    pub fn string() -> Self {
        Self::new("string")
    }

    pub fn boolean() -> Self {
        Self::new("boolean")
    }

    pub fn array(items: JsonSchema) -> Self {
        Self {
            schema_type: Some("array".to_string()),
            items: Some(Box::new(items)),
            ..Default::default()
        }
    }

    pub fn object() -> Self {
        Self::new("object")
    }

    pub fn with_nullable(mut self) -> Self {
        self.nullable = Some(true);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_minimum(mut self, min: impl Into<serde_json::Number>) -> Self {
        self.minimum = Some(min.into());
        self
    }

    pub fn with_maximum(mut self, max: impl Into<serde_json::Number>) -> Self {
        self.maximum = Some(max.into());
        self
    }

    pub fn with_min_length(mut self, min: usize) -> Self {
        self.min_length = Some(min);
        self
    }

    pub fn with_max_length(mut self, max: usize) -> Self {
        self.max_length = Some(max);
        self
    }

    pub fn with_enum_values(mut self, values: Vec<String>) -> Self {
        self.enum_values = Some(values);
        self
    }

    pub fn with_property(mut self, name: impl Into<String>, schema: JsonSchema) -> Self {
        let properties = self.properties.get_or_insert_with(serde_json::Map::new);
        properties.insert(
            name.into(),
            serde_json::to_value(schema).expect("Failed to serialize schema"),
        );
        self
    }

    pub fn with_required(mut self, required: Vec<String>) -> Self {
        self.required = Some(required);
        self
    }
}

impl From<JsonSchemaInline> for JsonSchema {
    fn from(inline: JsonSchemaInline) -> Self {
        JsonSchema::Inline(inline)
    }
}

impl From<JsonSchemaRef> for JsonSchema {
    fn from(ref_schema: JsonSchemaRef) -> Self {
        JsonSchema::Ref(ref_schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openrpc_document_serialization() {
        let doc = OpenRpcDocument::new("Test API", "1.0.0")
            .with_description("A test API")
            .with_method(
                MethodObject::new(
                    "get_item",
                    ContentDescriptor::new("result", JsonSchemaInline::object().into()),
                )
                .with_param(
                    ContentDescriptor::new("id", JsonSchemaInline::integer().into()).required(),
                ),
            );

        let json = serde_json::to_string_pretty(&doc).unwrap();
        assert!(json.contains("\"openrpc\": \"1.3.2\""));
        assert!(json.contains("\"name\": \"get_item\""));
    }

    #[test]
    fn test_json_schema_with_constraints() {
        let schema = JsonSchemaInline::integer()
            .with_minimum(serde_json::Number::from(1))
            .with_maximum(serde_json::Number::from(100));

        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["type"], "integer");
        assert_eq!(json["minimum"], 1);
        assert_eq!(json["maximum"], 100);
    }

    #[test]
    fn test_component_ref() {
        let ref_schema = JsonSchemaRef::component("Todo");
        let json = serde_json::to_value(&ref_schema).unwrap();
        assert_eq!(json["$ref"], "#/components/schemas/Todo");
    }
}
