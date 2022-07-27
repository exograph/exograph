use crate::graphql::execution_error::ExecutionError;

use crate::graphql::execution::resolver::FieldResolver;
use crate::graphql::execution::system_context::SystemContext;
use crate::graphql::introspection::definition::root_element::IntrospectionRootElement;
use crate::graphql::introspection::schema::{
    MUTATION_ROOT_TYPENAME, QUERY_ROOT_TYPENAME, SUBSCRIPTION_ROOT_TYPENAME,
};
use crate::graphql::request_context::RequestContext;
use crate::graphql::validation::field::ValidatedField;
use async_graphql_parser::types::{BaseType, OperationType, Type};
use async_graphql_value::{ConstValue, Name};
use async_trait::async_trait;
use serde_json::Value;

use super::resolver_support::Resolver;

#[async_trait]
impl<'a> FieldResolver<Value> for IntrospectionRootElement<'a> {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
        if system_context.allow_introspection {
            match self.name {
                "__type" => Ok(resolve_type(system_context, field, request_context).await?),
                "__schema" => Ok(system_context
                    .schema
                    .resolve_value(&field.subfields, system_context, request_context)
                    .await?),
                "__typename" => {
                    let typename = match self.operation_type {
                        OperationType::Query => QUERY_ROOT_TYPENAME,
                        OperationType::Mutation => MUTATION_ROOT_TYPENAME,
                        OperationType::Subscription => SUBSCRIPTION_ROOT_TYPENAME,
                    };
                    Ok(Value::String(typename.to_string()))
                }
                _ => {
                    return Err(ExecutionError::Generic(format!(
                        "No such introspection field {}",
                        self.name
                    )))
                }
            }
        } else {
            return Err(ExecutionError::Generic(
                "Introspection is not allowed".into(),
            ));
        }
    }
}

async fn resolve_type<'b>(
    system_context: &'b SystemContext,
    field: &ValidatedField,
    request_context: &'b RequestContext<'b>,
) -> Result<Value, ExecutionError> {
    let type_name = &field
        .arguments
        .iter()
        .find(|arg| arg.0 == "name")
        .unwrap()
        .1;

    if let ConstValue::String(name_specified) = &type_name {
        let tpe: Type = Type {
            base: BaseType::Named(Name::new(name_specified)),
            nullable: true,
        };
        tpe.resolve_value(&field.subfields, system_context, request_context)
            .await
    } else {
        Ok(Value::Null)
    }
}
