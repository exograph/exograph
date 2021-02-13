use std::sync::Arc;

use graphql_parser::schema::{Field, InputValue, TypeDefinition};
pub trait FieldDefinitionProvider<'a> {
    fn field_definition(&self) -> Field<'a, String>;
}

pub trait TypeDefinitionProvider {
    fn type_definition(&self) -> TypeDefinition<String>;
}

impl<T> TypeDefinitionProvider for Arc<T>
where
    T: TypeDefinitionProvider,
{
    fn type_definition(&self) -> TypeDefinition<String> {
        self.as_ref().type_definition()
    }
}
pub trait InputValueProvider<'a> {
    fn input_value(&self) -> InputValue<'a, String>;
}
