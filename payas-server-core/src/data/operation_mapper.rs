use crate::execution::system_context::QueryResponseBody;
use crate::execution_error::{DatabaseExecutionError, ExecutionError};
use crate::request_context::RequestContext;

use payas_sql::{
    AbstractInsert, AbstractOperation, AbstractPredicate, AbstractSelect, AbstractUpdate,
};

use tokio_postgres::{types::FromSqlOwned, Row};

use crate::{
    execution::system_context::{QueryResponse, SystemContext},
    validation::field::ValidatedField,
};

use async_graphql_value::ConstValue;
use payas_model::model::{
    mapped_arena::SerializableSlabIndex, operation::Mutation, service::ServiceMethod,
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
