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

use crate::openrpc::{
    Components, ContentDescriptor, JsonSchema, JsonSchemaInline, JsonSchemaRef, MethodObject,
    OpenRpcDocument,
};
use crate::schema::{
    RpcComponents, RpcMethod, RpcObjectType, RpcParameter, RpcSchema, RpcTypeSchema,
};

/// Convert an RpcSchema to an OpenRPC document.
pub fn to_openrpc(schema: &RpcSchema, title: &str, version: &str) -> OpenRpcDocument {
    let mut doc = OpenRpcDocument::new(title, version);

    // Convert methods
    for method in &schema.methods {
        doc.methods.push(convert_method(method));
    }

    // Convert component schemas
    if !schema.components.schemas.is_empty() {
        doc.components = Some(convert_components(&schema.components));
    }

    doc
}

fn convert_method(method: &RpcMethod) -> MethodObject {
    let result_schema = convert_type_schema(&method.result);
    let mut method_obj = MethodObject::new(
        &method.name,
        ContentDescriptor::new("result", result_schema),
    );

    if let Some(desc) = &method.description {
        method_obj = method_obj.with_description(desc);
    }

    for param in &method.params {
        method_obj = method_obj.with_param(convert_parameter(param));
    }

    method_obj
}

fn convert_parameter(param: &RpcParameter) -> ContentDescriptor {
    let schema = convert_type_schema(&param.schema);
    let mut descriptor = ContentDescriptor::new(&param.name, schema);

    if let Some(desc) = &param.description {
        descriptor = descriptor.with_description(desc);
    }

    if param.is_required() {
        descriptor = descriptor.required();
    } else {
        descriptor = descriptor.optional();
    }

    descriptor
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

        RpcTypeSchema::Optional { inner } => {
            let inner_schema = convert_type_schema(inner);
            match inner_schema {
                JsonSchema::Inline(inline) => JsonSchema::Inline(inline.with_nullable()),
                JsonSchema::Ref(ref_schema) => {
                    // For refs, we just return the ref as-is
                    // In a full implementation, we'd use oneOf/anyOf for nullable refs
                    JsonSchema::Ref(ref_schema)
                }
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
        "Uuid" => JsonSchemaInline::string(),
        "DateTime" => JsonSchemaInline::string(),
        "LocalDateTime" => JsonSchemaInline::string(),
        "LocalDate" => JsonSchemaInline::string(),
        "LocalTime" => JsonSchemaInline::string(),
        "Instant" => JsonSchemaInline::string(),
        "Json" => JsonSchemaInline::default(), // Any type
        "Blob" => JsonSchemaInline::string(),
        "Decimal" => JsonSchemaInline::string(), // Decimals are strings for precision
        _ => JsonSchemaInline::default(),        // Unknown type
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
        let field_schema = convert_type_schema(&field.schema);

        // Check if field is required (not optional)
        if !matches!(field.schema, RpcTypeSchema::Optional { .. }) {
            required_fields.push(field.name.clone());
        }

        schema = schema.with_property(&field.name, field_schema);
    }

    if !required_fields.is_empty() {
        schema = schema.with_required(required_fields);
    }

    JsonSchema::Inline(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{RpcMethod, RpcObjectField, RpcObjectType, RpcParameter, RpcSchema};
    use core_model::types::IntConstraints;

    #[test]
    fn test_convert_simple_schema() {
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

        let doc = to_openrpc(&schema, "Test API", "1.0.0");

        assert_eq!(doc.openrpc, "1.3.2");
        assert_eq!(doc.info.title, "Test API");
        assert_eq!(doc.methods.len(), 1);
        assert_eq!(doc.methods[0].name, "get_todos");
    }

    #[test]
    fn test_convert_with_validation() {
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
    fn test_convert_optional_parameter() {
        let param = RpcParameter::new(
            "filter",
            RpcTypeSchema::optional(RpcTypeSchema::scalar("String")),
        );

        let descriptor = convert_parameter(&param);

        assert_eq!(descriptor.required, Some(false));
    }
}
