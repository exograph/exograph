use async_trait::async_trait;

use crate::data::root_element::DataRootElement;
use crate::execution::query_response::{QueryResponse, QueryResponseBody};
use crate::execution::resolver::FieldResolver;
use crate::execution_error::ExecutionError;
use crate::introspection::definition::root_element::IntrospectionRootElement;
use crate::request_context::RequestContext;
use crate::validation::field::ValidatedField;
use crate::validation::operation::ValidatedOperation;
use crate::SystemContext;

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
