use async_graphql_parser::types::{BaseType, Type, TypeDefinition};
use async_trait::async_trait;
use serde_json::Value;

use crate::execution::resolver::{FieldResolver, Resolver};
use crate::execution_error::ExecutionError;
use crate::introspection::definition::type_introspection::TypeDefinitionIntrospection;
use crate::request_context::RequestContext;
use crate::{execution::system_context::SystemContext, validation::field::ValidatedField};

#[derive(Debug)]
struct BoxedType<'a> {
    tpe: &'a Type,
    type_kind: &'a str,
}

#[async_trait]
impl FieldResolver<Value> for TypeDefinition {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
        match field.name.as_str() {
            "name" => Ok(Value::String(self.name())),
            "kind" => Ok(Value::String(self.kind())),
            "description" => Ok(self.description().map(Value::String).unwrap_or(Value::Null)),
            "fields" => {
                self.fields()
                    .resolve_value(&field.subfields, system_context, request_context)
                    .await
            }
            "interfaces" => Ok(Value::Array(vec![])), // TODO
            "possibleTypes" => Ok(Value::Null),       // TODO
            "enumValues" => {
                self.enum_values()
                    .resolve_value(&field.subfields, system_context, request_context)
                    .await
            }
            "inputFields" => {
                self.input_fields()
                    .resolve_value(&field.subfields, system_context, request_context)
                    .await
            }
            "ofType" => Ok(Value::Null),
            "specifiedByUrl" => Ok(Value::Null),
            "__typename" => Ok(Value::String("__Type".to_string())),
            field_name => Err(ExecutionError::InvalidField(
                field_name.to_owned(),
                "TypeDefinition",
            )),
        }
    }
}

#[async_trait]
impl FieldResolver<Value> for Type {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
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
            boxed_type
                .resolve_field(field, system_context, request_context)
                .await
        } else {
            match base_type {
                BaseType::Named(name) => {
                    // See commented out derivation of FieldResolver for Option<T>
                    //system_context.schema.get_type_definition(name).resolve_field(system_context, field)
                    let tpe = system_context.schema.get_type_definition(name);
                    match tpe {
                        Some(tpe) => {
                            tpe.resolve_field(field, system_context, request_context)
                                .await
                        }
                        None => Ok(Value::Null),
                    }
                }
                BaseType::List(underlying) => {
                    let boxed_type = BoxedType {
                        tpe: underlying,
                        type_kind: "LIST",
                    };
                    boxed_type
                        .resolve_field(field, system_context, request_context)
                        .await
                }
            }
        }
    }
}

/// Resolver for a boxed (non-null or list type). Since the underlying type
/// determines the `ofType` value and the type_kind determines the `kind`, all
/// other fields evaluate to null
#[async_trait]
impl<'a> FieldResolver<Value> for BoxedType<'a> {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
        match field.name.as_str() {
            "kind" => Ok(Value::String(self.type_kind.to_owned())),
            "ofType" => {
                self.tpe
                    .resolve_value(&field.subfields, system_context, request_context)
                    .await
            }
            "name" | "description" | "specifiedByUrl" | "fields" | "interfaces"
            | "possibleTypes" | "enumValues" | "inoutFields" => Ok(Value::Null),
            field_name => Err(ExecutionError::InvalidField(
                field_name.to_owned(),
                "List/NonNull type",
            )),
        }
    }
}
