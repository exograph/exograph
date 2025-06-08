use std::sync::Arc;

use async_trait::async_trait;

use common::context::RequestContext;

use core_resolver::access_solver::AccessSolver;
use core_resolver::plugin::SubsystemRpcResolver;
use core_resolver::plugin::subsystem_rpc_resolver::{SubsystemRpcError, SubsystemRpcResponse};
use core_resolver::{QueryResponse, QueryResponseBody};
use exo_sql::{
    AbstractOperation, AbstractSelect, AliasedSelectionElement, DatabaseExecutor, Selection,
    SelectionCardinality, SelectionElement,
};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_resolver::database_helper::extractor;
use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;
use postgres_rpc_model::operation::PostgresOperationKind;
use postgres_rpc_model::{operation::PostgresOperation, subsystem::PostgresRpcSubsystemWithRouter};

pub struct PostgresSubsystemRpcResolver {
    pub id: &'static str,
    pub subsystem: PostgresRpcSubsystemWithRouter,
    pub executor: Arc<DatabaseExecutor>,
    pub api_path_prefix: String,
}

#[async_trait]
impl SubsystemRpcResolver for PostgresSubsystemRpcResolver {
    fn id(&self) -> &'static str {
        "postgres"
    }

    async fn resolve<'a>(
        &self,
        request_method: &str,
        _request_params: &Option<serde_json::Value>,
        request_context: &'a RequestContext<'a>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        let operation = self.subsystem.method_operation_map.get(request_method);

        if let Some(operation) = operation {
            let operation = operation.resolve(request_context, &self.subsystem).await?;

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
                .map_err(|e| from_postgres_error(PostgresExecutionError::Postgres(e)))?;

            let body = if result.len() == 1 {
                let string_result: String =
                    extractor(result.swap_remove(0)).map_err(from_postgres_error)?;
                Ok(QueryResponseBody::Raw(Some(string_result)))
            } else if result.is_empty() {
                Ok(QueryResponseBody::Raw(None))
            } else {
                Err(PostgresExecutionError::NonUniqueResult(result.len()))
            }
            .map_err(from_postgres_error)?;

            return Ok(Some(SubsystemRpcResponse {
                response: QueryResponse {
                    body,
                    headers: vec![],
                },
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
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractOperation, SubsystemRpcError>;
}

#[async_trait]
impl OperationResolver for PostgresOperation {
    async fn resolve<'a>(
        &self,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractOperation, SubsystemRpcError> {
        let entity_types = &subsystem.core_subsystem.entity_types;

        let entity_type = &entity_types[self.entity_type_id];

        let access_expr = {
            let access_expr_index = match self.kind {
                PostgresOperationKind::Query => entity_type.access.read,
                _ => {
                    return Err(SubsystemRpcError::UserDisplayError(
                        "Only queries are supported for this operation".to_string(),
                    ));
                }
            };
            &subsystem.core_subsystem.database_access_expressions[access_expr_index]
        };

        let access_predicate = subsystem
            .core_subsystem
            .solve(request_context, None, access_expr)
            .await
            .map_err(|_| SubsystemRpcError::Authorization)?
            .map(|p| p.0)
            .resolve();

        let selection = Selection::Json(
            entity_type
                .fields
                .iter()
                .filter_map(|field| match field.relation {
                    PostgresRelation::Scalar { column_id, .. } => {
                        Some(AliasedSelectionElement::new(
                            field.name.clone(),
                            SelectionElement::Physical(column_id),
                        ))
                    }
                    _ => None,
                })
                .collect(),
            SelectionCardinality::Many,
        );

        let select = AbstractSelect {
            table_id: entity_type.table_id,
            selection,
            predicate: access_predicate,
            order_by: None,
            offset: None,
            limit: None,
        };

        Ok(AbstractOperation::Select(select))
    }
}

fn from_postgres_error(e: PostgresExecutionError) -> SubsystemRpcError {
    match e {
        PostgresExecutionError::Authorization => SubsystemRpcError::Authorization,
        _ => SubsystemRpcError::UserDisplayError(e.user_error_message()),
    }
}
