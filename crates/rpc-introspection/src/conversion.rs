// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Conversion utilities between RPC schema types and OpenRPC types.

use core_model::types::TypeValidation;

use crate::rpc_schema_doc::{
    Components, ContentDescriptor, JsonSchema, JsonSchemaInline, JsonSchemaRef, MethodObject,
    MethodParams, RpcDocument,
};
use crate::schema::{RpcComponents, RpcMethod, RpcObjectType, RpcSchema, RpcTypeSchema};

/// Controls the output format for schema generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaGeneration {
    /// OpenRPC spec-compliant: params as ContentDescriptor array with per-param `required`
    OpenRpc,
    /// MCP/LLM-optimized: params as JSON Schema object with `properties` + `required` array
    Mcp,
}

/// Convert an RpcSchema to a format-agnostic RPC document.
pub fn to_rpc_document(schema: &RpcSchema, mode: SchemaGeneration) -> RpcDocument {
    let mut doc = RpcDocument::new();

    // Convert methods
    for method in &schema.methods {
        doc.methods.push(convert_method(method, mode));
    }

    // Convert component schemas
    if !schema.components.schemas.is_empty() {
        doc.components = Some(convert_components(&schema.components));
    }

    doc
}

fn convert_method(method: &RpcMethod, mode: SchemaGeneration) -> MethodObject {
    let result_schema = convert_type_schema(&method.result);
    let mut method_obj = MethodObject::new(
        &method.name,
        ContentDescriptor::new("result", result_schema),
    );

    if let Some(desc) = &method.description {
        method_obj = method_obj.with_description(desc);
    }

    match mode {
        SchemaGeneration::OpenRpc => {
            for param in &method.params {
                // Unwrap Optional: optionality conveyed by `required: false`
                let (schema, is_optional) = unwrap_optional(&param.schema);
                let schema = convert_type_schema(schema);

                let mut descriptor = ContentDescriptor::new(&param.name, schema);
                if let Some(desc) = &param.description {
                    descriptor = descriptor.with_description(desc);
                }
                if is_optional {
                    descriptor = descriptor.optional();
                } else {
                    descriptor = descriptor.required();
                }
                method_obj = method_obj.with_param(descriptor);
            }
        }
        SchemaGeneration::Mcp => {
            let mut params_schema = JsonSchemaInline::object();
            let mut required_params = Vec::new();

            for param in &method.params {
                // Unwrap Optional: optionality conveyed by not being in `required` array
                let (inner, is_optional) = unwrap_optional(&param.schema);
                let schema = convert_type_schema(inner);

                // Move param description into the property schema
                let schema = match &param.description {
                    Some(desc) => schema.with_description(desc),
                    None => schema,
                };

                if !is_optional {
                    required_params.push(param.name.clone());
                }
                params_schema = params_schema.with_property(&param.name, schema);
            }

            if !required_params.is_empty() {
                params_schema = params_schema.with_required(required_params);
            }

            method_obj.params = MethodParams::Schema(Box::new(JsonSchema::Inline(params_schema)));
        }
    }

    method_obj
}

/// Unwrap an Optional schema, returning the inner schema and whether it was optional.
fn unwrap_optional(schema: &RpcTypeSchema) -> (&RpcTypeSchema, bool) {
    match schema {
        RpcTypeSchema::Optional { inner } => (inner, true),
        other => (other, false),
    }
}

fn convert_type_schema(schema: &RpcTypeSchema) -> JsonSchema {
    match schema {
        RpcTypeSchema::Scalar {
            type_name,
            validation,
        } => {
            let mut inline = type_name_to_json_schema(type_name);

            if let Some(validation) = validation {
                inline = apply_validation(inline, validation);
            }

            JsonSchema::Inline(inline)
        }

        RpcTypeSchema::Enum { values } => {
            JsonSchema::Inline(JsonSchemaInline::string().with_enum_values(values.clone()))
        }

        RpcTypeSchema::Object { type_ref } => JsonSchema::Ref(JsonSchemaRef::component(type_ref)),

        RpcTypeSchema::Array { items } => {
            let items_schema = convert_type_schema(items);
            JsonSchema::Inline(JsonSchemaInline::array(items_schema))
        }

        RpcTypeSchema::OneOf { variants } => {
            let convert_variant = |v: &crate::schema::OneOfVariant| match v {
                crate::schema::OneOfVariant::Inline {
                    properties,
                    required,
                } => {
                    let mut obj = JsonSchemaInline::object();
                    for (name, schema) in properties {
                        obj = obj.with_property(name, convert_type_schema(schema));
                    }
                    // additionalProperties: false ensures each variant only accepts its
                    // declared fields, enabling unambiguous variant matching.
                    // For example, without this, if we have an entity with `id` as pk
                    // and unique `username`, `{"id": 1, "username": "alice"}` would
                    // match both the `(id)` and `(username)` variants.
                    obj = obj
                        .with_required(required.clone())
                        .with_additional_properties(false);
                    JsonSchema::Inline(obj)
                }
                crate::schema::OneOfVariant::Ref(type_ref) => {
                    JsonSchema::Ref(JsonSchemaRef::component(type_ref))
                }
            };

            if variants.len() == 1 {
                // Single variant: emit the object schema directly without oneOf wrapper
                convert_variant(&variants[0])
            } else {
                let one_of_schemas: Vec<JsonSchema> =
                    variants.iter().map(convert_variant).collect();
                JsonSchema::Inline(JsonSchemaInline {
                    one_of: Some(one_of_schemas),
                    ..Default::default()
                })
            }
        }

        RpcTypeSchema::Optional { inner } => {
            // This arm is now only reached for return types (params/fields unwrap Optional before calling)
            let inner_schema = convert_type_schema(inner);
            match inner_schema {
                JsonSchema::Inline(inline) => JsonSchema::Inline(inline.with_nullable()),
                JsonSchema::Ref(ref_schema) => JsonSchema::Ref(ref_schema.with_nullable()),
            }
        }
    }
}

fn type_name_to_json_schema(type_name: &str) -> JsonSchemaInline {
    match type_name {
        "Int" => JsonSchemaInline::integer(),
        "Float" => JsonSchemaInline::number(),
        "String" => JsonSchemaInline::string(),
        "Boolean" => JsonSchemaInline::boolean(),
        "ID" => JsonSchemaInline::string(),
        "Uuid" => JsonSchemaInline::string().with_format("uuid"),
        "DateTime" => JsonSchemaInline::string().with_format("date-time"),
        "LocalDateTime" => JsonSchemaInline::string().with_format("date-time"),
        "LocalDate" => JsonSchemaInline::string().with_format("date"),
        "LocalTime" => JsonSchemaInline::string().with_format("time"),
        "Instant" => JsonSchemaInline::string().with_format("date-time"),
        "Json" => JsonSchemaInline::default(), // Any type
        "Blob" => JsonSchemaInline::string().with_format("byte"), // Base64-encoded binary
        "Decimal" => JsonSchemaInline::string(), // Decimals are strings for precision
        "Vector" => JsonSchemaInline::array(JsonSchema::Inline(JsonSchemaInline::number())), // Array of floats
        _ => JsonSchemaInline::default(), // Unknown type
    }
}

fn apply_validation(mut schema: JsonSchemaInline, validation: &TypeValidation) -> JsonSchemaInline {
    match validation {
        TypeValidation::Int(constraints) => {
            if let Some(min) = constraints.min {
                schema = schema.with_minimum(serde_json::Number::from(min));
            }
            if let Some(max) = constraints.max {
                schema = schema.with_maximum(serde_json::Number::from(max));
            }
        }
        TypeValidation::Float(constraints) => {
            if let Some(min) = constraints.min
                && let Some(num) = serde_json::Number::from_f64(min)
            {
                schema = schema.with_minimum(num);
            }
            if let Some(max) = constraints.max
                && let Some(num) = serde_json::Number::from_f64(max)
            {
                schema = schema.with_maximum(num);
            }
        }
        TypeValidation::String(constraints) => {
            if let Some(min_length) = constraints.min_length {
                schema = schema.with_min_length(min_length);
            }
            if let Some(max_length) = constraints.max_length {
                schema = schema.with_max_length(max_length);
            }
        }
    }
    schema
}

fn convert_components(components: &RpcComponents) -> Components {
    let mut openrpc_components = Components::new();

    for (name, obj_type) in &components.schemas {
        let schema = convert_object_type(obj_type);
        openrpc_components = openrpc_components.with_schema(name, schema);
    }

    openrpc_components
}

fn convert_object_type(obj_type: &RpcObjectType) -> JsonSchema {
    let mut schema = JsonSchemaInline::object();

    if let Some(desc) = &obj_type.description {
        schema = schema.with_description(desc);
    }

    let mut required_fields = Vec::new();

    for field in &obj_type.fields {
        // Unwrap Optional: optionality conveyed by not being in `required` array
        let (inner, is_optional) = unwrap_optional(&field.schema);
        let field_schema = convert_type_schema(inner);

        if !is_optional {
            required_fields.push(field.name.clone());
        }

        schema = schema.with_property(&field.name, field_schema);
    }

    if !required_fields.is_empty() {
        schema = schema.with_required(required_fields);
    }

    if obj_type.additional_properties_false {
        schema = schema.with_additional_properties(false);
    }

    JsonSchema::Inline(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{RpcMethod, RpcObjectField, RpcObjectType, RpcSchema};
    use core_model::types::IntConstraints;

    #[test]
    fn convert_simple_schema() {
        let mut schema = RpcSchema::new();

        let todo_type = RpcObjectType::new("Todo")
            .with_field(RpcObjectField::new("id", RpcTypeSchema::scalar("Int")))
            .with_field(RpcObjectField::new(
                "title",
                RpcTypeSchema::scalar("String"),
            ));
        schema.add_object_type("Todo".to_string(), todo_type);

        let method = RpcMethod::new(
            "get_todos".to_string(),
            RpcTypeSchema::array(RpcTypeSchema::object("Todo")),
        );
        schema.add_method(method);

        let doc = to_rpc_document(&schema, SchemaGeneration::OpenRpc);

        assert_eq!(doc.methods.len(), 1);
        assert_eq!(doc.methods[0].name, "get_todos");
    }

    #[test]
    fn convert_with_validation() {
        let schema_type = RpcTypeSchema::scalar_with_validation(
            "Int",
            TypeValidation::Int(IntConstraints::from_range(1, 100)),
        );

        let json_schema = convert_type_schema(&schema_type);

        let json = serde_json::to_value(&json_schema).unwrap();
        assert_eq!(json["type"], "integer");
        assert_eq!(json["minimum"], 1);
        assert_eq!(json["maximum"], 100);
    }

    #[test]
    fn unwrap_optional_schema() {
        let schema = RpcTypeSchema::optional(RpcTypeSchema::scalar("String"));
        let (inner, is_optional) = unwrap_optional(&schema);
        assert!(is_optional);
        assert!(matches!(inner, RpcTypeSchema::Scalar { type_name, .. } if type_name == "String"));

        let schema = RpcTypeSchema::scalar("Int");
        let (inner, is_optional) = unwrap_optional(&schema);
        assert!(!is_optional);
        assert!(matches!(inner, RpcTypeSchema::Scalar { type_name, .. } if type_name == "Int"));
    }
}
