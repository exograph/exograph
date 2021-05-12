use async_graphql_parser::types::{FieldDefinition, InputValueDefinition, TypeDefinition};

use payas_model::model::system::ModelSystem;

pub trait FieldDefinitionProvider {
    fn field_definition(&self, system: &ModelSystem) -> FieldDefinition;
}

pub trait TypeDefinitionProvider {
    fn type_definition(&self, system: &ModelSystem) -> TypeDefinition;
}

pub trait InputValueProvider {
    fn input_value(&self) -> InputValueDefinition;
}
