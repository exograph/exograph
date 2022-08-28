use async_graphql_value::Name;

use crate::graphql::introspection::schema::{
    SchemaEnumValueDefinition, SchemaFieldDefinition, SchemaInputValueDefinition,
    SchemaTypeDefinition, SchemaTypeKind,
};

/// Deal with variants of `TypeDefinition` to give a uniform view suitable for introspection
pub trait TypeDefinitionIntrospection {
    fn name(&self) -> String;
    fn kind(&self) -> String;
    fn description(&self) -> Option<String>;
    fn fields(&self) -> Option<&Vec<SchemaFieldDefinition>>;
    fn interfaces(&self) -> Option<&Vec<Name>>;
    fn possible_types(&self) -> Option<&Vec<Name>>;
    fn enum_values(&self) -> Option<&Vec<SchemaEnumValueDefinition>>;
    fn input_fields(&self) -> Option<&Vec<SchemaInputValueDefinition>>;
}

impl TypeDefinitionIntrospection for SchemaTypeDefinition {
    fn name(&self) -> String {
        self.name.to_string()
    }

    fn kind(&self) -> String {
        match self.kind {
            SchemaTypeKind::Scalar => "SCALAR".to_owned(),
            SchemaTypeKind::Object(_) => "OBJECT".to_owned(),
            SchemaTypeKind::Enum(_) => "ENUM".to_owned(),
            SchemaTypeKind::InputObject(_) => "INPUT_OBJECT".to_owned(),
        }
    }

    fn description(&self) -> Option<String> {
        self.description.as_ref().map(|d| d.to_owned())
    }

    fn fields(&self) -> Option<&Vec<SchemaFieldDefinition>> {
        // Spec: return null except for ObjectType
        // TODO: includeDeprecated arg
        match &self.kind {
            SchemaTypeKind::Object(value) => Some(&value.fields),
            _ => None,
        }
    }

    fn interfaces(&self) -> Option<&Vec<Name>> {
        // Spec: return null except for ObjectType
        match &self.kind {
            SchemaTypeKind::Object(value) => Some(&value.implements),
            _ => None,
        }
    }

    fn possible_types(&self) -> Option<&Vec<Name>> {
        // Spec: return null except for UnionType and Interface
        // Since we don't (need to) support unions and interfaces, this is always None
        None
    }

    fn enum_values(&self) -> Option<&Vec<SchemaEnumValueDefinition>> {
        // Spec: return null except for EnumType
        match &self.kind {
            SchemaTypeKind::Enum(value) => Some(&value.values),
            _ => None,
        }
    }

    fn input_fields(&self) -> Option<&Vec<SchemaInputValueDefinition>> {
        // Spec: return null except for InputObjectType
        match &self.kind {
            SchemaTypeKind::InputObject(value) => Some(&value.fields),
            _ => None,
        }
    }
}
