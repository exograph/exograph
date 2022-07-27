use async_trait::async_trait;

use super::system_context::SystemContext;
use crate::graphql::data::root_element::DataRootElement;
use crate::graphql::execution::field_resolver::FieldResolver;
use crate::graphql::execution::query_response::{QueryResponse, QueryResponseBody};
use crate::graphql::execution_error::ExecutionError;
use crate::graphql::introspection::definition::root_element::IntrospectionRootElement;
use crate::graphql::request_context::RequestContext;
use crate::graphql::validation::field::ValidatedField;
use crate::graphql::validation::operation::ValidatedOperation;

#[async_trait]
impl FieldResolver<'static, QueryResponse, ExecutionError, SystemContext> for ValidatedOperation {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse, ExecutionError> {
        let name = field.name.as_str();

        if name.starts_with("__") {
            let introspection_root = IntrospectionRootElement {
                operation_type: &self.typ,
                name,
            };

            let body = introspection_root
                .resolve_field(field, system_context, request_context)
                .await?;

            Ok(QueryResponse {
                body: QueryResponseBody::Json(body),
                headers: vec![],
            })
        } else {
            let data_root = DataRootElement {
                system: &system_context.system,
                operation_type: &self.typ,
            };
            data_root
                .resolve(field, system_context, request_context)
                .await
        }
    }
}
