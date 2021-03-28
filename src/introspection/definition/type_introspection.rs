use async_graphql_parser::{
    types::{EnumValueDefinition, FieldDefinition, InputValueDefinition, TypeDefinition, TypeKind},
    Positioned,
};
use async_graphql_value::Name;

/// Deal with variants of `TypeDefinition` to give a uniform view suitable for introspection
pub trait TypeDefinitionIntrospection {
    fn name(&self) -> String;
    fn kind(&self) -> String;
    fn description(&self) -> Option<String>;
    fn fields(&self) -> Option<&Vec<Positioned<FieldDefinition>>>;
    fn interfaces(&self) -> Option<&Vec<Positioned<Name>>>;
    fn possible_types(&self) -> Option<&Vec<Positioned<Name>>>;
    fn enum_values(&self) -> Option<&Vec<Positioned<EnumValueDefinition>>>;
    fn input_fields(&self) -> Option<&Vec<Positioned<InputValueDefinition>>>;
}

impl TypeDefinitionIntrospection for TypeDefinition {
    fn name(&self) -> String {
        self.name.node.to_string()
    }

    fn kind(&self) -> String {
        match self.kind {
            TypeKind::Scalar => "SCALAR".to_owned(),
            TypeKind::Object(_) => "OBJECT".to_owned(),
            TypeKind::Interface(_) => "INTERFACE".to_owned(),
            TypeKind::Union(_) => "UNION".to_owned(),
            TypeKind::Enum(_) => "ENUM".to_owned(),
            TypeKind::InputObject(_) => "INPUT_OBJECT".to_owned(),
        }
    }

    fn description(&self) -> Option<String> {
        self.description.as_ref().map(|d| d.node.to_owned())
    }

    fn fields(&self) -> Option<&Vec<Positioned<FieldDefinition>>> {
        // Spec: return null except for ObjectType
        // TODO: includeDeprecated arg
        match &self.kind {
            TypeKind::Object(value) => Some(&value.fields),
            _ => None,
        }
    }

    fn interfaces(&self) -> Option<&Vec<Positioned<Name>>> {
        // Spec: return null except for ObjectType
        match &self.kind {
            TypeKind::Object(value) => Some(&value.implements),
            _ => None,
        }
    }

    fn possible_types(&self) -> Option<&Vec<Positioned<Name>>> {
        // Spec: return null except for UnionType and Interface
        match &self.kind {
            TypeKind::Union(value) => Some(&value.members),
            TypeKind::Interface(_value) => todo!(),
            _ => None,
        }
    }

    fn enum_values(&self) -> Option<&Vec<Positioned<EnumValueDefinition>>> {
        // Spec: return null except for EnumType
        match &self.kind {
            TypeKind::Enum(value) => Some(&value.values),
            _ => None,
        }
    }

    fn input_fields(&self) -> Option<&Vec<Positioned<InputValueDefinition>>> {
        // Spec: return null except for InputObjectType
        match &self.kind {
            TypeKind::InputObject(value) => Some(&value.fields),
            _ => None,
        }
    }
}
