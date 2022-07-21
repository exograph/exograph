use crate::execution::system_context::QueryResponseBody;
use crate::execution_error::{DatabaseExecutionError, ExecutionError};
use crate::request_context::RequestContext;
use async_trait::async_trait;

use payas_sql::{
    AbstractInsert, AbstractOperation, AbstractPredicate, AbstractSelect, AbstractUpdate,
};

use tokio_postgres::{types::FromSqlOwned, Row};

use crate::{
    execution::system_context::{QueryResponse, SystemContext},
    validation::field::ValidatedField,
};

use super::access_solver;
use crate::deno::interception::InterceptedOperation;
use crate::execution::resolver::FieldResolver;
use async_graphql_value::ConstValue;
use payas_model::model::{
    mapped_arena::SerializableSlabIndex,
    operation::{Interceptors, Mutation, OperationReturnType},
    service::ServiceMethod,
    GqlCompositeType, GqlTypeKind,
};

pub trait SQLMapper<'a, R> {
    fn map_to_sql(
        &'a self,
        argument: &'a ConstValue,
        system_context: &'a SystemContext,
    ) -> Result<R, ExecutionError>;
}

pub trait SQLInsertMapper<'a> {
    fn insert_operation(
        &'a self,
        mutation: &'a Mutation,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        system_context: &'a SystemContext,
    ) -> Result<AbstractInsert, ExecutionError>;
}

pub trait SQLUpdateMapper<'a> {
    fn update_operation(
        &'a self,
        mutation: &'a Mutation,
        predicate: AbstractPredicate<'a>,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        system_context: &'a SystemContext,
    ) -> Result<AbstractUpdate, ExecutionError>;
}

#[async_trait]
pub trait OperationResolver<'a> {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<OperationResolverResult<'a>, ExecutionError>;

    async fn execute(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<QueryResponse, ExecutionError> {
        let resolve = move |field: &'a ValidatedField,
                            system_context: &'a SystemContext,
                            request_context: &'a RequestContext<'a>| {
            self.resolve_operation(field, system_context, request_context)
        };

        let intercepted_operation =
            InterceptedOperation::new(self.name(), self.interceptors().ordered());
        let QueryResponse { body, headers } = intercepted_operation
            .execute(field, system_context, request_context, &resolve)
            .await?;

        // A proceed call in an around interceptor may have returned more fields that necessary (just like a normal service),
        // so we need to filter out the fields that are not needed.
        // TODO: Validate that all requested fields are present in the response.
        let field_selected_response_body = match body {
            QueryResponseBody::Json(value @ serde_json::Value::Object(_)) => {
                let resolved_set = value
                    .resolve_fields(&field.subfields, system_context, request_context)
                    .await?;
                QueryResponseBody::Json(serde_json::Value::Object(
                    resolved_set.into_iter().collect(),
                ))
            }
            _ => body,
        };

        Ok(QueryResponse {
            body: field_selected_response_body,
            headers,
        })
    }

    fn name(&self) -> &str;

    fn interceptors(&self) -> &Interceptors;
}

#[allow(clippy::large_enum_variant)]
pub enum OperationResolverResult<'a> {
    SQLOperation(AbstractOperation<'a>),
    DenoOperation(DenoOperation),
}

pub struct DenoOperation(pub SerializableSlabIndex<ServiceMethod>);

pub enum SQLOperationKind {
    Create,
    Retrieve,
    Update,
    Delete,
}

pub async fn compute_sql_access_predicate<'a>(
    return_type: &OperationReturnType,
    kind: &SQLOperationKind,
    system_context: &'a SystemContext,
    request_context: &'a RequestContext<'a>,
) -> AbstractPredicate<'a> {
    let return_type = return_type.typ(&system_context.system);

    match &return_type.kind {
        GqlTypeKind::Primitive => AbstractPredicate::True,
        GqlTypeKind::Composite(GqlCompositeType { access, .. }) => {
            let access_expr = match kind {
                SQLOperationKind::Create => &access.creation,
                SQLOperationKind::Retrieve => &access.read,
                SQLOperationKind::Update => &access.update,
                SQLOperationKind::Delete => &access.delete,
            };
            access_solver::solve_access(access_expr, request_context, &system_context.system).await
        }
    }
}

impl<'a> OperationResolverResult<'a> {
    pub async fn execute(
        &self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<QueryResponse, ExecutionError> {
        match self {
            OperationResolverResult::SQLOperation(abstract_operation) => {
                let mut result = system_context
                    .database_executor
                    .execute(abstract_operation)
                    .await
                    .map_err(DatabaseExecutionError::Database)?;

                let body = if result.len() == 1 {
                    let string_result = extractor(result.swap_remove(0))?;
                    Ok(QueryResponseBody::Raw(Some(string_result)))
                } else if result.is_empty() {
                    Ok(QueryResponseBody::Raw(None))
                } else {
                    Err(DatabaseExecutionError::NonUniqueResult(result.len()))
                }?;

                Ok(QueryResponse {
                    body,
                    headers: vec![], // we shouldn't get any HTTP headers from a SQL op
                })
            }

            OperationResolverResult::DenoOperation(operation) => {
                operation
                    .execute(field, system_context, request_context)
                    .await
            }
        }
    }
}

fn extractor<T: FromSqlOwned>(row: Row) -> Result<T, DatabaseExecutionError> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => Err(DatabaseExecutionError::EmptyRow(err)),
    }
}
