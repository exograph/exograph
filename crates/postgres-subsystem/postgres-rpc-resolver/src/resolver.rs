use std::sync::Arc;

use async_trait::async_trait;

use common::context::RequestContext;
use common::http::{Headers, ResponseBody, ResponsePayload};

use core_plugin_interface::core_resolver::plugin::{
    SubsystemResolutionError, SubsystemRpcResolver,
};
use exo_sql::{
    AbstractOperation, AbstractPredicate, AbstractSelect, AliasedSelectionElement,
    DatabaseExecutor, Selection, SelectionCardinality, SelectionElement,
};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_resolver::database_helper::extractor;
use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;
use postgres_rpc_model::{operation::PostgresOperation, subsystem::PostgresRpcSubsystemWithRouter};

pub struct PostgresSubsystemRpcResolver {
    #[allow(dead_code)]
    pub id: &'static str,
    pub subsystem: PostgresRpcSubsystemWithRouter,
    #[allow(dead_code)]
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
        request_context: &'a RequestContext<'a>,
    ) -> Result<Option<ResponsePayload>, SubsystemResolutionError> {
        use common::http::RequestPayload;

        let body = request_context.take_body();

        // TODO: Use a parser for JSON-RPC requests
        let operation_name = body.get("method").ok_or_else(|| {
            SubsystemResolutionError::UserDisplayError(
                "Invalid JSON-RPC request. No method provided.".to_string(),
            )
        })?;

        let operation_name = match operation_name.as_str() {
            Some(operation_name) => operation_name,
            None => {
                return Err(SubsystemResolutionError::UserDisplayError(
                    "Invalid JSON-RPC request. Method name is not a string.".to_string(),
                ));
            }
        };

        let operation = self.subsystem.method_operation_map.get(operation_name);

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
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractOperation, SubsystemResolutionError>;
}

#[async_trait]
impl OperationResolver for PostgresOperation {
    async fn resolve<'a>(
        &self,
        _request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractOperation, SubsystemResolutionError> {
        let entity_types = &subsystem.core_subsystem.entity_types;

        let entity_type = &entity_types[self.entity_type_id];

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
            predicate: AbstractPredicate::True,
            order_by: None,
            offset: None,
            limit: None,
        };

        Ok(AbstractOperation::Select(select))
    }
}
