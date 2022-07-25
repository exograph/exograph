use async_trait::async_trait;

use crate::data::data_resolver::DataResolver;
use crate::execution_error::ExecutionError;
use crate::introspection::definition::root_element::RootElement;
use crate::request_context::RequestContext;
use crate::validation::field::ValidatedField;
use crate::validation::operation::ValidatedOperation;

use super::query_response::{QueryResponse, QueryResponseBody};
use super::resolver::FieldResolver;
use super::system_context::SystemContext;

#[async_trait]
impl FieldResolver<QueryResponse> for ValidatedOperation {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse, ExecutionError> {
        let name = field.name.as_str();

        if name.starts_with("__") {
            let introspection_root = RootElement {
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
            system_context
                .system
                .resolve(field, &self.typ, system_context, request_context)
                .await
        }
    }
}
