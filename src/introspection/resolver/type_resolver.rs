use async_graphql_parser::{Positioned, types::{BaseType, Field, Type, TypeDefinition}};
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;
use crate::introspection::definition::type_introspection::TypeDefinitionIntrospection;

#[derive(Debug)]
struct BoxedType<'a> {
    tpe: &'a Type,
    type_kind: &'a str,
}

impl<'a> FieldResolver<Value> for TypeDefinition {
    fn resolve_field(&self, query_context: &QueryContext<'_>, field: &Positioned<Field>) -> Value {
        match field.node.name.node.as_str() {
            "name" => Value::String(self.name().to_owned()),
            "kind" => Value::String(self.kind()),
            "description" => self
                .description()
                .clone()
                .map(|v| Value::String(v))
                .unwrap_or(Value::Null),
            "fields" => self
                .fields()
                .resolve_value(query_context, &field.node.selection_set),
            "interfaces" => Value::Array(vec![]), // TODO
            "possibleTypes" => Value::Null,       // TODO
            "enumValues" => self
                .enum_values()
                .resolve_value(query_context, &field.node.selection_set),
            "inputFields" => self
                .input_fields()
                .resolve_value(query_context, &field.node.selection_set),
            "ofType" => Value::Null,
            "specifiedByUrl" => Value::Null,
            field_name => todo!("Invalid field {:?} for TypeDefinition", field_name), // TODO: Make it a proper error
        }
    }
}

impl FieldResolver<Value> for Type {
    fn resolve_field(&self, query_context: &QueryContext<'_>, field: &Positioned<Field>) -> Value {
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
                        None => Value::Null,
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
    fn resolve_field(&self, query_context: &QueryContext<'_>, field: &Positioned<Field>) -> Value {
        match field.node.name.node.as_str() {
            "kind" => Value::String(self.type_kind.to_owned()),
            "ofType" => self
                .tpe
                .resolve_value(query_context, &field.node.selection_set),
            "name" | "description" | "specifiedByUrl" | "fields" | "interfaces"
            | "possibleTypes" | "enumValues" | "inoutFields" => Value::Null,
            field_name => todo!("Invalid field {:?} for List/NonNull type", field_name), // TODO: Make it a proper error
        }
    }
}
