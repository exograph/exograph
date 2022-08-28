use payas_model::model::system::ModelSystem;

use crate::graphql::introspection::schema::{
    SchemaFieldDefinition, SchemaInputValueDefinition, SchemaTypeDefinition,
};

pub trait FieldDefinitionProvider {
    fn field_definition(&self, system: &ModelSystem) -> SchemaFieldDefinition;
}

pub trait TypeDefinitionProvider {
    fn type_definition(&self, system: &ModelSystem) -> SchemaTypeDefinition;
}

pub trait InputValueProvider {
    fn input_value(&self) -> SchemaInputValueDefinition;
}
