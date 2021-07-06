use async_graphql_parser::{
    types::{BaseType, Field, Type, TypeDefinition},
    Positioned,
};
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;
use crate::introspection::definition::type_introspection::TypeDefinitionIntrospection;
use anyhow::{anyhow, Result};

#[derive(Debug)]
struct BoxedType<'a> {
    tpe: &'a Type,
    type_kind: &'a str,
}

impl<'a> FieldResolver<Value> for TypeDefinition {
    fn resolve_field(
        &self,
        query_context: &QueryContext<'_>,
        field: &Positioned<Field>,
    ) -> Result<Value> {
        match field.node.name.node.as_str() {
            "name" => Ok(Value::String(self.name())),
            "kind" => Ok(Value::String(self.kind())),
            "description" => Ok(self.description().map(Value::String).unwrap_or(Value::Null)),
            "fields" => self
                .fields()
                .resolve_value(query_context, &field.node.selection_set),
            "interfaces" => Ok(Value::Array(vec![])), // TODO
            "possibleTypes" => Ok(Value::Null),       // TODO
            "enumValues" => self
                .enum_values()
                .resolve_value(query_context, &field.node.selection_set),
            "inputFields" => self
                .input_fields()
                .resolve_value(query_context, &field.node.selection_set),
            "ofType" => Ok(Value::Null),
            "specifiedByUrl" => Ok(Value::Null),
            field_name => Err(anyhow!(GraphQLExecutionError::InvalidField(
                field_name.to_owned(),
                "TypeDefinition",
            ))),
        }
    }
}

impl FieldResolver<Value> for Type {
    fn resolve_field(
        &self,
        query_context: &QueryContext<'_>,
        field: &Positioned<Field>,
    ) -> Result<Value> {
        let base_type = &self.base;

        if !self.nullable {
            let underlying = Type {
                base: base_type.to_owned(),
                nullable: true, // Now the underlying type is nullable
            };
            let boxed_type = BoxedType {
                tpe: &underlying,
                type_kind: "NON_NULL",
            };
            boxed_type.resolve_field(query_context, field)
        } else {
            match base_type {
                BaseType::Named(name) => {
                    // See commented out derivation of FieldResolver for Option<T>
                    //query_context.schema.get_type_definition(name).resolve_field(query_context, field)
                    let tpe = query_context.schema.get_type_definition(name);
                    match tpe {
                        Some(tpe) => tpe.resolve_field(query_context, field),
                        None => Ok(Value::Null),
                    }
                }
                BaseType::List(underlying) => {
                    let boxed_type = BoxedType {
                        tpe: underlying,
                        type_kind: "LIST",
                    };
                    boxed_type.resolve_field(query_context, field)
                }
            }
        }
    }
}

// Resolver for boxed (non-null or list type). Since the underlying type determines the `ofType` value and the type_kind determines the `kind`,
// all other fields evaluates to null
impl<'a> FieldResolver<Value> for BoxedType<'a> {
    fn resolve_field(
        &self,
        query_context: &QueryContext<'_>,
        field: &Positioned<Field>,
    ) -> Result<Value> {
        match field.node.name.node.as_str() {
            "kind" => Ok(Value::String(self.type_kind.to_owned())),
            "ofType" => self
                .tpe
                .resolve_value(query_context, &field.node.selection_set),
            "name" | "description" | "specifiedByUrl" | "fields" | "interfaces"
            | "possibleTypes" | "enumValues" | "inoutFields" => Ok(Value::Null),
            field_name => Err(anyhow!(GraphQLExecutionError::InvalidField(
                field_name.to_owned(),
                "List/NonNull type",
            ))),
        }
    }
}
