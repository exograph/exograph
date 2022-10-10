use async_graphql_parser::types::Directive;
use async_trait::async_trait;
use payas_core_resolver::{
    introspection::definition::schema::Schema, plugin::SubsystemResolutionError,
    request_context::RequestContext,
};
use serde_json::Value;

use payas_core_resolver::validation::field::ValidatedField;

use crate::field_resolver::FieldResolver;

#[async_trait]
impl FieldResolver<Value, SubsystemResolutionError> for Directive {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        _schema: &Schema,
        _request_context: &'e RequestContext<'e>,
    ) -> Result<Value, SubsystemResolutionError> {
        match field.name.as_str() {
            "name" => Ok(Value::String(self.name.node.as_str().to_owned())),
            "description" => Ok(Value::Null),
            "isRepeatable" => Ok(Value::Bool(false)), // TODO
            "locations" => Ok(Value::Array(vec![])),  // TODO
            "args" => Ok(Value::Array(vec![])),       // TODO
            "__typename" => Ok(Value::String("__Directive".to_string())),
            field_name => Err(SubsystemResolutionError::InvalidField(
                field_name.to_owned(),
                "Directive",
            )),
        }
    }
}
