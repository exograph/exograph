use async_trait::async_trait;

use payas_core_resolver::validation::field::ValidatedField;
use payas_core_resolver::validation::operation::ValidatedOperation;
use payas_core_resolver::{request_context::RequestContext, QueryResponse};

use super::system_context::SystemContext;
use crate::graphql::{
    data::data_root_element::DataRootElement, execution::field_resolver::FieldResolver,
    execution_error::ExecutionError,
};

/// Resolver for the root operation.
///
/// The operation may be a query or a mutation and may be for data or for introspection.
///
#[async_trait]
impl FieldResolver<QueryResponse, ExecutionError, SystemContext> for ValidatedOperation {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse, ExecutionError> {
        let name = field.name.as_str();

        if name.starts_with("__") {
            todo!()

        //     let introspection_root = IntrospectionRootElement {
        //         operation_type: &self.typ,
        //         name,
        //     };

        //     let body = introspection_root
        //         .resolve_field(field, system_context, request_context)
        //         .await?;

        //     Ok(QueryResponse {
        //         body: QueryResponseBody::Json(body),
        //         headers: vec![],
        //     })
        } else {
            let data_root = DataRootElement {
                operation_type: &self.typ,
            };
            data_root
                .resolve(field, system_context, request_context)
                .await
        }
    }
}
