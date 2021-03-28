use async_graphql_parser::types::{FieldDefinition, InputValueDefinition, TypeDefinition};

pub trait FieldDefinitionProvider {
    fn field_definition(&self) -> FieldDefinition;
}

pub trait TypeDefinitionProvider {
    fn type_definition(&self) -> TypeDefinition;
}

pub trait InputValueProvider {
    fn input_value(&self) -> InputValueDefinition;
}
