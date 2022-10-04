use payas_core_resolver::validation::field::ValidatedField;
use payas_core_resolver::{plugin::SubsystemResolutionError, request_context::RequestContext};

use async_trait::async_trait;
use payas_core_resolver::introspection::definition::schema::{
    Schema, MUTATION_ROOT_TYPENAME, QUERY_ROOT_TYPENAME, SUBSCRIPTION_ROOT_TYPENAME,
};
use serde_json::Value;

use crate::field_resolver::FieldResolver;

use super::resolver_support::Resolver;

#[async_trait]
impl FieldResolver<Value, SubsystemResolutionError> for Schema {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        schema: &Schema,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, SubsystemResolutionError> {
        match field.name.as_str() {
            "types" => {
                self.type_definitions
                    .resolve_value(&field.subfields, schema, request_context)
                    .await
            }
            "queryType" => {
                self.get_type_definition(QUERY_ROOT_TYPENAME)
                    .resolve_value(&field.subfields, schema, request_context)
                    .await
            }
            "mutationType" => {
                self.get_type_definition(MUTATION_ROOT_TYPENAME)
                    .resolve_value(&field.subfields, schema, request_context)
                    .await
            }
            "subscriptionType" => {
                self.get_type_definition(SUBSCRIPTION_ROOT_TYPENAME)
                    .resolve_value(&field.subfields, schema, request_context)
                    .await
            }
            "directives" => Ok(Value::Array(vec![])), // TODO
            "description" => Ok(Value::String("Top-level schema".to_string())),
            "__typename" => Ok(Value::String("__Schema".to_string())),
            field_name => Err(SubsystemResolutionError::InvalidField(
                field_name.to_owned(),
                "Schema",
            )),
        }
    }
}
