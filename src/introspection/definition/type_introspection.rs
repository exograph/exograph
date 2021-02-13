use graphql_parser::schema::{EnumValue, Field, InputValue, TypeDefinition};

/// Deal with variants of `TypeDefinition` to give a uniform view suitable for introspection
pub trait TypeDefinitionIntrospection<'a> {
    fn name(&self) -> String;
    fn kind(&self) -> String;
    fn description(&self) -> Option<String>;
    fn fields(&self) -> Option<&'a Vec<Field<String>>>;
    fn interfaces(&self) -> Option<&Vec<String>>;
    fn possible_types(&'a self) -> Option<&'a Vec<String>>;
    fn enum_values(&self) -> Option<&'a Vec<EnumValue<String>>>;
    fn input_fields(&self) -> Option<&'a Vec<InputValue<String>>>;
}

impl<'a> TypeDefinitionIntrospection<'a> for TypeDefinition<'a, String> {
    fn name(&self) -> String {
        match self {
            TypeDefinition::Scalar(value) => value.name.to_owned(),
            TypeDefinition::Object(value) => value.name.to_owned(),
            TypeDefinition::Interface(value) => value.name.to_owned(),
            TypeDefinition::Union(value) => value.name.to_owned(),
            TypeDefinition::Enum(value) => value.name.to_owned(),
            TypeDefinition::InputObject(value) => value.name.to_owned(),
        }
    }

    fn kind(&self) -> String {
        match self {
            TypeDefinition::Scalar(_) => "SCALAR".to_owned(),
            TypeDefinition::Object(_) => "OBJECT".to_owned(),
            TypeDefinition::Interface(_) => "INTERFACE".to_owned(),
            TypeDefinition::Union(_) => "UNION".to_owned(),
            TypeDefinition::Enum(_) => "ENUM".to_owned(),
            TypeDefinition::InputObject(_) => "INPUT_OBJECT".to_owned(),
        }
    }

    fn description(&self) -> Option<String> {
        match self {
            TypeDefinition::Scalar(value) => value.description.to_owned(),
            TypeDefinition::Object(value) => value.description.to_owned(),
            TypeDefinition::Interface(value) => value.description.to_owned(),
            TypeDefinition::Union(value) => value.description.to_owned(),
            TypeDefinition::Enum(value) => value.description.to_owned(),
            TypeDefinition::InputObject(value) => value.description.to_owned(),
        }
    }

    fn fields(&self) -> Option<&'a Vec<Field<String>>> {
        // Spec: return null except for ObjectType
        // TODO: includeDeprecated arg
        match self {
            TypeDefinition::Object(value) => Some(&value.fields),
            _ => None,
        }
    }

    fn interfaces(&self) -> Option<&Vec<String>> {
        // Spec: return null except for ObjectType
        match self {
            TypeDefinition::Object(value) => Some(&value.implements_interfaces),
            _ => None,
        }
    }

    fn possible_types(&'a self) -> Option<&'a Vec<String>> {
        // Spec: return null except for UnionType and Interface
        match self {
            TypeDefinition::Union(value) => Some(&value.types),
            TypeDefinition::Interface(_value) => todo!(),
            _ => None,
        }
    }

    fn enum_values(&self) -> Option<&'a Vec<EnumValue<String>>> {
        // Spec: return null except for EnumType
        match self {
            TypeDefinition::Enum(value) => Some(&value.values),
            _ => None,
        }
    }

    fn input_fields(&self) -> Option<&'a Vec<InputValue<String>>> {
        // Spec: return null except for InputObjectType
        match self {
            TypeDefinition::InputObject(value) => Some(&value.fields),
            _ => None,
        }
    }
}
