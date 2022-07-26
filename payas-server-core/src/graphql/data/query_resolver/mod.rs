use crate::graphql::{
    data::database::database_query::DatabaseQuery, execution::system_context::SystemContext,
    execution_error::ExecutionError, request_context::RequestContext,
    validation::field::ValidatedField,
};

use async_trait::async_trait;
use payas_model::model::operation::{Interceptors, Query, QueryKind};
use payas_sql::{AbstractOperation, AbstractPredicate};

use super::{
    operation_mapper::{DenoOperation, OperationResolverResult},
    operation_resolver::OperationResolver,
};

// TODO: deal with panics at the type level

#[async_trait]
impl<'a> OperationResolver<'a> for Query {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<OperationResolverResult<'a>, ExecutionError> {
        Ok(match &self.kind {
            QueryKind::Database(query_params) => {
                let database_query = DatabaseQuery {
                    return_type: &self.return_type,
                    query_params,
                };
                let operation = database_query
                    .compute_select(
                        field,
                        AbstractPredicate::True,
                        system_context,
                        request_context,
                    )
                    .await?;

                OperationResolverResult::SQLOperation(AbstractOperation::Select(operation))
            }

            QueryKind::Service { method_id, .. } => {
                OperationResolverResult::DenoOperation(DenoOperation(method_id.unwrap()))
            }
        })
    }

    fn interceptors(&self) -> &Interceptors {
        &self.interceptors
    }

    fn name(&self) -> &str {
        &self.name
    }
}
