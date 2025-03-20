use std::sync::Arc;

use async_trait::async_trait;

use common::context::RequestContext;
use common::http::{Headers, RequestPayload, ResponseBody, ResponsePayload};

use core_resolver::plugin::{SubsystemResolutionError, SubsystemRestResolver};
use exo_sql::{
    AbstractOperation, AbstractPredicate, AbstractSelect, DatabaseExecutor, SelectionCardinality,
};
use postgres_core_resolver::database_helper::extractor;
use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;
use postgres_rest_model::{
    operation::PostgresOperation, subsystem::PostgresRestSubsystemWithRouter,
};

pub struct PostgresSubsystemRestResolver {
    #[allow(dead_code)]
    pub id: &'static str,
    pub subsystem: PostgresRestSubsystemWithRouter,
    #[allow(dead_code)]
    pub executor: Arc<DatabaseExecutor>,
    pub api_path_prefix: String,
}

#[async_trait]
impl SubsystemRestResolver for PostgresSubsystemRestResolver {
    fn id(&self) -> &'static str {
        "postgres"
    }

    async fn resolve<'a>(
        &self,
        request_context: &'a RequestContext<'a>,
    ) -> Result<Option<ResponsePayload>, SubsystemResolutionError> {
        let operation = self
            .subsystem
            .find_matching(request_context.get_head(), &self.api_path_prefix);

        if let Some(operation) = operation {
            let operation = operation.resolve(request_context).await?;

            let mut tx = request_context
                .system_context
                .transaction_holder
                .try_lock()
                .unwrap();

            let mut result = self
                .executor
                .execute(
                    operation,
                    &mut tx,
                    &self.subsystem.core_subsystem.as_ref().database,
                )
                .await
                .map_err(PostgresExecutionError::Postgres)?;

            let body = if result.len() == 1 {
                let string_result: String = extractor(result.swap_remove(0))?;
                Ok(ResponseBody::Bytes(string_result.into()))
            } else if result.is_empty() {
                Ok(ResponseBody::None)
            } else {
                Err(PostgresExecutionError::NonUniqueResult(result.len()))
            }?;

            return Ok(Some(ResponsePayload {
                body,
                headers: Headers::new(),
                status_code: http::StatusCode::OK,
            }));
        }

        Ok(None)
    }
}

#[async_trait]
trait OperationResolver {
    async fn resolve<'a>(
        &self,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractOperation, SubsystemResolutionError>;
}

#[async_trait]
impl OperationResolver for PostgresOperation {
    async fn resolve<'a>(
        &self,
        _request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractOperation, SubsystemResolutionError> {
        let select = AbstractSelect {
            table_id: self.table_id,
            selection: exo_sql::Selection::Json(vec![], SelectionCardinality::Many),
            predicate: AbstractPredicate::True,
            order_by: None,
            offset: None,
            limit: None,
        };

        Ok(AbstractOperation::Select(select))
    }
}
