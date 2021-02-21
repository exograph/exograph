use graphql_parser::{
    query::Field,
    schema::{Type, TypeDefinition},
};
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;
use crate::introspection::definition::type_introspection::TypeDefinitionIntrospection;

#[derive(Debug)]
struct BoxedType<'a> {
    tpe: Type<'a, String>,
    type_kind: &'a str,
}

impl<'a> FieldResolver for TypeDefinition<'a, String> {
    fn resolve_field(&self, query_context: &QueryContext<'_>, field: &Field<'_, String>) -> Value {
        match field.name.as_str() {
            "name" => Value::String(self.name().to_owned()),
            "kind" => Value::String(self.kind()),
            "description" => self
                .description()
                .clone()
                .map(|v| Value::String(v))
                .unwrap_or(Value::Null),
            "fields" => {
                // TODO: Why can't we use self.fields() here and in enumValues/inputFields
                let fields = match self {
                    TypeDefinition::Object(value) => Some(&value.fields),
                    _ => None,
                };
                fields.resolve_value(query_context, &field.selection_set)
            }
            "interfaces" => Value::Array(vec![]), // TODO
            "possibleTypes" => Value::Null,       // TODO
            "enumValues" => {
                let enum_values = match self {
                    TypeDefinition::Enum(ref value) => Some(&value.values),
                    _ => None,
                };
                enum_values.resolve_value(query_context, &field.selection_set)
            }
            "inputFields" => {
                let fields = match self {
                    TypeDefinition::InputObject(ref value) => Some(&value.fields),
                    _ => None,
                };
                fields.resolve_value(query_context, &field.selection_set)
            }
            "ofType" => Value::Null,
            field_name => todo!("Invalid field {:?} for TypeDefinition", field_name), // TODO: Make it a proper error
        }
    }
}

impl<'a> FieldResolver for Type<'a, String> {
    fn resolve_field(&self, query_context: &QueryContext<'_>, field: &Field<'_, String>) -> Value {
        match self {
            Type::NamedType(name) => {
                // See commented out derivation of FieldResolver for Option<T>
                //query_context.schema.get_type_definition(name).resolve_field(query_context, field)
                let tpe = query_context.schema.get_type_definition(name);
                match tpe {
                    Some(tpe) => tpe.resolve_field(query_context, field),
                    None => Value::Null,
                }
            }
            Type::ListType(undelying) => {
                let boxed_type = BoxedType {
                    tpe: undelying.as_ref().to_owned(),
                    type_kind: "LIST",
                };
                boxed_type.resolve_field(query_context, field)
            }
            Type::NonNullType(undelying) => {
                let boxed_type = BoxedType {
                    tpe: undelying.as_ref().to_owned(),
                    type_kind: "NON_NULL",
                };
                boxed_type.resolve_field(query_context, field)
            }
        }
    }
}

impl<'a> FieldResolver for BoxedType<'a> {
    fn resolve_field(&self, query_context: &QueryContext<'_>, field: &Field<'_, String>) -> Value {
        match field.name.as_str() {
            "kind" => Value::String(self.type_kind.to_owned()),
            "ofType" => self.tpe.resolve_value(query_context, &field.selection_set),
            "name" => Value::Null,
            field_name => todo!("Invalid field {:?} for List/NonNull type", field_name), // TODO: Make it a proper error
        }
    }
}
