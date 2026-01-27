// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use async_trait::async_trait;

use common::context::RequestContext;

use core_model::types::OperationReturnType;
use core_resolver::access_solver::AccessSolver;
use core_resolver::plugin::SubsystemRpcResolver;
use core_resolver::plugin::subsystem_rpc_resolver::{SubsystemRpcError, SubsystemRpcResponse};
use core_resolver::{QueryResponse, QueryResponseBody};
use exo_sql::{
    AbstractOperation, AbstractOrderBy, AbstractPredicate, AbstractSelect, AliasedSelectionElement,
    DatabaseExecutor, Limit, Offset, Selection, SelectionCardinality, SelectionElement,
};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_model::types::EntityType;
use postgres_core_resolver::database_helper::extractor;
use postgres_core_resolver::order_by_mapper::compute_order_by;
use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;
use postgres_core_resolver::predicate_mapper::compute_predicate;
use postgres_core_resolver::predicate_util::json_to_val;
use postgres_rpc_model::operation::{CollectionQuery, PkQuery};
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;

use common::value::Val;

pub struct PostgresSubsystemRpcResolver {
    #[allow(dead_code)]
    pub id: &'static str,
    pub subsystem: PostgresRpcSubsystemWithRouter,
    pub executor: Arc<DatabaseExecutor>,
    #[allow(dead_code)]
    pub api_path_prefix: String,
}

/// Enum to represent the resolved operation (either collection or pk query)
enum ResolvedOperation<'a> {
    CollectionQuery(&'a CollectionQuery),
    PkQuery(&'a PkQuery),
}

#[async_trait]
impl SubsystemRpcResolver for PostgresSubsystemRpcResolver {
    fn id(&self) -> &'static str {
        "postgres"
    }

    async fn resolve<'a>(
        &self,
        request_method: &str,
        request_params: &Option<serde_json::Value>,
        request_context: &'a RequestContext<'a>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        let resolved_operation = self
            .subsystem
            .pk_queries
            .get_by_key(request_method)
            .map(ResolvedOperation::PkQuery)
            .or_else(|| {
                self.subsystem
                    .collection_queries
                    .get_by_key(request_method)
                    .map(ResolvedOperation::CollectionQuery)
            });

        if let Some(resolved_operation) = resolved_operation {
            let operation = match resolved_operation {
                ResolvedOperation::CollectionQuery(query) => {
                    query
                        .resolve(request_params, request_context, &self.subsystem)
                        .await?
                }
                ResolvedOperation::PkQuery(query) => {
                    query
                        .resolve(request_params, request_context, &self.subsystem)
                        .await?
                }
            };

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

/// Trait for resolving operations to an AbstractSelect.
/// Similar to GraphQL's OperationSelectionResolver pattern.
#[async_trait]
trait OperationSelectionResolver {
    async fn resolve_select<'a>(
        &'a self,
        request_params: &Option<serde_json::Value>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractSelect, SubsystemRpcError>;
}

/// Trait for resolving operations to an AbstractOperation.
/// Blanket impl wraps AbstractSelect in AbstractOperation::Select.
#[async_trait]
trait OperationResolver {
    async fn resolve<'a>(
        &'a self,
        request_params: &Option<serde_json::Value>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractOperation, SubsystemRpcError>;
}

/// Blanket implementation: any OperationSelectionResolver is also an OperationResolver
#[async_trait]
impl<T: OperationSelectionResolver + Send + Sync> OperationResolver for T {
    async fn resolve<'a>(
        &'a self,
        request_params: &Option<serde_json::Value>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractOperation, SubsystemRpcError> {
        self.resolve_select(request_params, request_context, subsystem)
            .await
            .map(AbstractOperation::Select)
    }
}

/// Shared function to compute the final AbstractSelect.
/// Similar to GraphQL's compute_select pattern.
fn compute_select(
    predicate: AbstractPredicate,
    order_by: Option<AbstractOrderBy>,
    limit: Option<Limit>,
    offset: Option<Offset>,
    entity_type: &EntityType,
    return_type: &OperationReturnType<EntityType>,
) -> AbstractSelect {
    let selection_cardinality = match return_type {
        OperationReturnType::List(_) => SelectionCardinality::Many,
        _ => SelectionCardinality::One,
    };

    let selection = Selection::Json(
        entity_type
            .fields
            .iter()
            .filter_map(|field| match field.relation {
                PostgresRelation::Scalar { column_id, .. } => Some(AliasedSelectionElement::new(
                    field.name.clone(),
                    SelectionElement::Physical(column_id),
                )),
                _ => None,
            })
            .collect(),
        selection_cardinality,
    );

    AbstractSelect {
        table_id: entity_type.table_id,
        selection,
        predicate,
        order_by,
        offset,
        limit,
    }
}

/// Helper to compute access predicate for an entity type
async fn compute_access_predicate<'a>(
    entity_type: &EntityType,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<AbstractPredicate, SubsystemRpcError> {
    let access_expr =
        &subsystem.core_subsystem.database_access_expressions[entity_type.access.read];

    subsystem
        .core_subsystem
        .solve(request_context, None, access_expr)
        .await
        .map_err(|_| SubsystemRpcError::Authorization)
        .map(|result| result.map(|p| p.0).resolve())
}

#[async_trait]
impl OperationSelectionResolver for CollectionQuery {
    async fn resolve_select<'a>(
        &'a self,
        request_params: &Option<serde_json::Value>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractSelect, SubsystemRpcError> {
        let entity_type = self.return_type.typ(&subsystem.core_subsystem.entity_types);

        let access_predicate =
            compute_access_predicate(entity_type, request_context, subsystem).await?;

        // Extract the "where" parameter and compute the user predicate
        let user_predicate = match extract_param(request_params, "where") {
            Some(where_val) => compute_predicate(
                &self.parameters.predicate_param,
                &where_val,
                &subsystem.core_subsystem,
                request_context,
            )
            .await
            .map_err(from_postgres_error)?,
            None => AbstractPredicate::True,
        };

        // Extract the "orderBy" parameter and compute the order by clause
        let order_by = match extract_param(request_params, "orderBy") {
            Some(order_by_val) => Some(
                compute_order_by(
                    &self.parameters.order_by_param,
                    &order_by_val,
                    &subsystem.core_subsystem,
                    request_context,
                )
                .await
                .map_err(from_postgres_error)?,
            ),
            None => None,
        };

        // Combine user predicate with access predicate
        let predicate = AbstractPredicate::and(user_predicate, access_predicate);

        // Extract limit and offset parameters
        let limit = extract_limit_offset(request_params, "limit").map(Limit);
        let offset = extract_limit_offset(request_params, "offset").map(Offset);

        Ok(compute_select(
            predicate,
            order_by,
            limit,
            offset,
            entity_type,
            &self.return_type,
        ))
    }
}

#[async_trait]
impl OperationSelectionResolver for PkQuery {
    async fn resolve_select<'a>(
        &'a self,
        request_params: &Option<serde_json::Value>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractSelect, SubsystemRpcError> {
        let entity_type = self.return_type.typ(&subsystem.core_subsystem.entity_types);

        let access_predicate =
            compute_access_predicate(entity_type, request_context, subsystem).await?;

        // Compute predicate from pk parameters - each pk field is an implicit equals
        let mut pk_predicates = Vec::new();
        for predicate_param in &self.parameters.predicate_params {
            let param_value =
                extract_param(request_params, &predicate_param.name).ok_or_else(|| {
                    SubsystemRpcError::UserDisplayError(format!(
                        "Missing required parameter: {}",
                        predicate_param.name
                    ))
                })?;

            // Use compute_predicate which handles ImplicitEqual properly
            let predicate = compute_predicate(
                predicate_param,
                &param_value,
                &subsystem.core_subsystem,
                request_context,
            )
            .await
            .map_err(from_postgres_error)?;

            pk_predicates.push(predicate);
        }

        // Combine all pk predicates with AND
        let pk_predicate = pk_predicates
            .into_iter()
            .reduce(AbstractPredicate::and)
            .unwrap_or(AbstractPredicate::True);

        // Combine pk predicate with access predicate
        let predicate = AbstractPredicate::and(pk_predicate, access_predicate);

        Ok(compute_select(
            predicate,
            None, // No ordering for single item
            None, // No limit for single item
            None, // No offset for single item
            entity_type,
            &self.return_type,
        ))
    }
}

fn from_postgres_error(e: PostgresExecutionError) -> SubsystemRpcError {
    match e {
        PostgresExecutionError::Authorization => SubsystemRpcError::Authorization,
        _ => SubsystemRpcError::UserDisplayError(e.user_error_message()),
    }
}

/// Extract an optional parameter from request params and convert to Val.
fn extract_param(request_params: &Option<serde_json::Value>, key: &str) -> Option<Val> {
    request_params
        .as_ref()
        .and_then(|params| params.get(key))
        .map(json_to_val)
}

/// Extract a limit or offset value from request parameters.
fn extract_limit_offset(request_params: &Option<serde_json::Value>, key: &str) -> Option<i64> {
    request_params
        .as_ref()
        .and_then(|params| params.get(key))
        .and_then(|v| v.as_i64())
}
