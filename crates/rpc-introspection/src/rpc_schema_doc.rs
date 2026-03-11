// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Format-agnostic RPC schema document types shared between OpenRPC and MCP formats.

use serde::{Deserialize, Serialize};

/// Format-agnostic RPC schema document shared between OpenRPC and MCP formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcDocument {
    /// The available RPC methods
    pub methods: Vec<MethodObject>,
    /// Reusable schema components
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Components>,
}

impl RpcDocument {
    pub fn new() -> Self {
        Self {
            methods: Vec::new(),
            components: None,
        }
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

impl Default for RpcDocument {
    fn default() -> Self {
        Self::new()
    }
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
    pub params: MethodParams,
    /// The result of calling this method
    pub result: ContentDescriptor,
}

/// Method parameters in either OpenRPC or MCP format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MethodParams {
    /// OpenRPC-style: array of content descriptors
    Descriptors(Vec<ContentDescriptor>),
    /// MCP-style: JSON Schema object with properties + required
    Schema(Box<JsonSchema>),
}

impl MethodObject {
    pub fn new(name: impl Into<String>, result: ContentDescriptor) -> Self {
        Self {
            name: name.into(),
            description: None,
            summary: None,
            params: MethodParams::Descriptors(Vec::new()),
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
        match &mut self.params {
            MethodParams::Descriptors(descriptors) => descriptors.push(param),
            MethodParams::Schema(_) => {
                panic!("Cannot add ContentDescriptor param to MCP-style method params")
            }
        }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nullable: Option<bool>,
}

impl JsonSchemaRef {
    pub fn new(ref_path: impl Into<String>) -> Self {
        Self {
            ref_path: ref_path.into(),
            description: None,
            nullable: None,
        }
    }

    /// Create a reference to a schema in the components section
    pub fn component(name: impl Into<String>) -> Self {
        Self {
            ref_path: format!("#/components/schemas/{}", name.into()),
            description: None,
            nullable: None,
        }
    }

    pub fn with_nullable(mut self) -> Self {
        self.nullable = Some(true);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
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

    /// Format hint for strings (e.g., "uuid", "date", "date-time", "time", "byte")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    /// Whether additional properties are allowed for objects
    #[serde(
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_properties: Option<bool>,

    /// oneOf schemas (for variant matching)
    #[serde(rename = "oneOf", skip_serializing_if = "Option::is_none")]
    pub one_of: Option<Vec<JsonSchema>>,
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

    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
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

    pub fn with_additional_properties(mut self, value: bool) -> Self {
        self.additional_properties = Some(value);
        self
    }
}

impl JsonSchema {
    pub fn with_description(self, description: impl Into<String>) -> Self {
        match self {
            JsonSchema::Ref(r) => JsonSchema::Ref(r.with_description(description)),
            JsonSchema::Inline(i) => JsonSchema::Inline(i.with_description(description)),
        }
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
    fn json_schema_with_constraints() {
        let schema = JsonSchemaInline::integer()
            .with_minimum(serde_json::Number::from(1))
            .with_maximum(serde_json::Number::from(100));

        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["type"], "integer");
        assert_eq!(json["minimum"], 1);
        assert_eq!(json["maximum"], 100);
    }

    #[test]
    fn component_ref() {
        let ref_schema = JsonSchemaRef::component("Todo");
        let json = serde_json::to_value(&ref_schema).unwrap();
        assert_eq!(json["$ref"], "#/components/schemas/Todo");
        assert!(json.get("nullable").is_none());
        assert!(json.get("description").is_none());
    }

    #[test]
    fn nullable_ref() {
        let ref_schema = JsonSchemaRef::component("Todo").with_nullable();
        let json = serde_json::to_value(&ref_schema).unwrap();
        assert_eq!(json["$ref"], "#/components/schemas/Todo");
        assert_eq!(json["nullable"], true);
    }

    #[test]
    fn ref_with_description() {
        let ref_schema = JsonSchemaRef::component("Filter").with_description("A filter");
        let json = serde_json::to_value(&ref_schema).unwrap();
        assert_eq!(json["$ref"], "#/components/schemas/Filter");
        assert_eq!(json["description"], "A filter");
        assert!(json.get("nullable").is_none());
    }

    #[test]
    fn nullable_ref_with_description() {
        let ref_schema = JsonSchemaRef::component("Todo")
            .with_nullable()
            .with_description("Optional todo");
        let json = serde_json::to_value(&ref_schema).unwrap();
        assert_eq!(json["$ref"], "#/components/schemas/Todo");
        assert_eq!(json["nullable"], true);
        assert_eq!(json["description"], "Optional todo");
    }

    #[test]
    fn openrpc_style_params() {
        let method = MethodObject::new(
            "get_items",
            ContentDescriptor::new("result", JsonSchemaInline::object().into()),
        )
        .with_param(ContentDescriptor::new("id", JsonSchemaInline::integer().into()).required())
        .with_param(
            ContentDescriptor::new(
                "filter",
                JsonSchema::Ref(JsonSchemaRef::component("Filter")),
            )
            .optional(),
        );

        let json = serde_json::to_value(&method).unwrap();
        let params = json["params"].as_array().unwrap();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0]["name"], "id");
        assert_eq!(params[0]["required"], true);
        assert_eq!(params[0]["schema"]["type"], "integer");
        assert_eq!(params[1]["name"], "filter");
        assert_eq!(params[1]["required"], false);
        assert_eq!(params[1]["schema"]["$ref"], "#/components/schemas/Filter");
    }

    #[test]
    fn mcp_style_params() {
        let params_schema = JsonSchemaInline::object()
            .with_property("id", JsonSchema::Inline(JsonSchemaInline::integer()))
            .with_property(
                "filter",
                JsonSchema::Ref(
                    JsonSchemaRef::component("Filter").with_description("Filter conditions"),
                ),
            )
            .with_required(vec!["id".to_string()]);

        let mut method = MethodObject::new(
            "get_items",
            ContentDescriptor::new("result", JsonSchemaInline::object().into()),
        );
        method.params = MethodParams::Schema(Box::new(JsonSchema::Inline(params_schema)));

        let json = serde_json::to_value(&method).unwrap();
        let params = &json["params"];
        assert_eq!(params["type"], "object");
        assert_eq!(params["properties"]["id"]["type"], "integer");
        assert_eq!(
            params["properties"]["filter"]["$ref"],
            "#/components/schemas/Filter"
        );
        assert_eq!(
            params["properties"]["filter"]["description"],
            "Filter conditions"
        );
        let required = params["required"].as_array().unwrap();
        assert_eq!(required, &["id"]);
    }

    #[test]
    fn object_with_required_fields() {
        let schema = JsonSchemaInline::object()
            .with_property("id", JsonSchema::Inline(JsonSchemaInline::integer()))
            .with_property("name", JsonSchema::Inline(JsonSchemaInline::string()))
            .with_property("bio", JsonSchema::Inline(JsonSchemaInline::string()))
            .with_required(vec!["id".to_string(), "name".to_string()]);

        let json = serde_json::to_value(&schema).unwrap();
        let required = json["required"].as_array().unwrap();
        assert_eq!(required, &["id", "name"]);
        // "bio" is not required — optionality conveyed by absence from required array
        assert!(!required.contains(&serde_json::Value::String("bio".to_string())));
        // No nullable on any field
        assert!(json["properties"]["id"].get("nullable").is_none());
        assert!(json["properties"]["bio"].get("nullable").is_none());
    }

    #[test]
    fn object_with_ref_field_not_required() {
        // Optional $ref field: just a plain $ref, not in required array (no oneOf, no nullable)
        let schema = JsonSchemaInline::object()
            .with_property("id", JsonSchema::Inline(JsonSchemaInline::integer()))
            .with_property(
                "filter",
                JsonSchema::Ref(JsonSchemaRef::component("IntFilter")),
            )
            .with_required(vec!["id".to_string()]);

        let json = serde_json::to_value(&schema).unwrap();
        // filter is a plain $ref with no oneOf wrapper
        assert_eq!(
            json["properties"]["filter"]["$ref"],
            "#/components/schemas/IntFilter"
        );
        assert!(json["properties"]["filter"].get("oneOf").is_none());
        assert!(json["properties"]["filter"].get("nullable").is_none());
    }

    #[test]
    fn nullable_return_type_ref() {
        // Return type nullable $ref: uses nullable: true on the ref itself
        let result = ContentDescriptor::new(
            "result",
            JsonSchema::Ref(JsonSchemaRef::component("Todo").with_nullable()),
        );

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["schema"]["$ref"], "#/components/schemas/Todo");
        assert_eq!(json["schema"]["nullable"], true);
        // No oneOf wrapper
        assert!(json["schema"].get("oneOf").is_none());
    }

    #[test]
    fn nullable_return_type_inline() {
        let result = ContentDescriptor::new(
            "result",
            JsonSchema::Inline(JsonSchemaInline::integer().with_nullable()),
        );

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["schema"]["type"], "integer");
        assert_eq!(json["schema"]["nullable"], true);
    }
}
