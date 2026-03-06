// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;

use common::context::RequestContext;
use common::value::Val;

use core_model::types::OperationReturnType;
use core_resolver::access_solver::AccessSolver;
use core_resolver::plugin::SubsystemRpcResolver;
use core_resolver::plugin::subsystem_rpc_resolver::{SubsystemRpcError, SubsystemRpcResponse};
use core_resolver::{QueryResponse, QueryResponseBody};
use exo_sql::{
    AbstractOperation, AbstractOrderBy, AbstractPredicate, AbstractSelect, AliasedSelectionElement,
    DatabaseExecutor, Limit, Offset, RelationId, Selection, SelectionCardinality, SelectionElement,
};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_model::types::{EntityType, PostgresField};
use postgres_core_resolver::database_helper::extractor;
use postgres_core_resolver::order_by_mapper::compute_order_by;
use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;
use postgres_core_resolver::predicate_mapper::compute_predicate;
use postgres_rpc_model::operation::{CollectionQuery, PkQuery, UniqueQuery};
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;
use rpc_introspection::RpcSchema;

pub struct PostgresSubsystemRpcResolver {
    #[allow(dead_code)]
    pub id: &'static str,
    pub subsystem: PostgresRpcSubsystemWithRouter,
    pub executor: Arc<DatabaseExecutor>,
    #[allow(dead_code)]
    pub api_path_prefix: String,
    rpc_schema: RpcSchema,
}

impl PostgresSubsystemRpcResolver {
    pub fn new(
        id: &'static str,
        subsystem: PostgresRpcSubsystemWithRouter,
        executor: Arc<DatabaseExecutor>,
        api_path_prefix: String,
    ) -> Self {
        let rpc_schema = crate::schema_builder::build_rpc_schema(&subsystem);
        Self {
            id,
            subsystem,
            executor,
            api_path_prefix,
            rpc_schema,
        }
    }
}

/// Enum to represent the resolved operation (either collection, pk, or unique query)
enum ResolvedOperation<'a> {
    Collection(&'a CollectionQuery),
    Pk(&'a PkQuery),
    Unique(&'a UniqueQuery),
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
        // Check if we handle this method at all
        let Some(rpc_method) = self.rpc_schema.method(request_method) else {
            return Ok(None);
        };

        // Validate params against schema first (catches unknown params, type mismatches)
        let mut validated_params = rpc_method
            .parse_params(request_params, &self.rpc_schema.components)
            .map_err(|e| SubsystemRpcError::InvalidParams(e.user_message()))?;

        // First try collection queries (these have distinct method names like get_todos)
        let resolved_operation = self
            .subsystem
            .collection_queries
            .get_by_key(request_method)
            .map(ResolvedOperation::Collection);

        // If not a collection query, try PK + unique queries (which share the same method name)
        let resolved_operation = if resolved_operation.is_some() {
            resolved_operation
        } else {
            let pk_query = self.subsystem.pk_queries.get_by_key(request_method);

            // Collect unique queries with matching method name
            let unique_queries: Vec<&UniqueQuery> = self
                .subsystem
                .unique_queries
                .iter()
                .filter(|(_, q)| q.name == request_method)
                .map(|(_, q)| q)
                .collect();

            if pk_query.is_some() || !unique_queries.is_empty() {
                // Extract the `by` param — it's a Val::Object containing the actual lookup fields
                let by_val = validated_params.remove("by").ok_or_else(|| {
                    SubsystemRpcError::InvalidParams("Missing required parameter: by".to_string())
                })?;

                let by_fields = match by_val {
                    Val::Object(fields) => fields,
                    _ => {
                        return Err(SubsystemRpcError::InvalidParams(
                            "'by' parameter must be an object".to_string(),
                        ));
                    }
                };

                // Replace validated_params with the unwrapped by fields
                validated_params = by_fields;

                let provided_param_names: Vec<String> = validated_params.keys().cloned().collect();

                Some(find_matching_get_query(
                    pk_query,
                    &unique_queries,
                    &provided_param_names,
                )?)
            } else {
                None
            }
        };

        if let Some(resolved_operation) = resolved_operation {
            let operation = match resolved_operation {
                ResolvedOperation::Collection(query) => {
                    query
                        .resolve(&mut validated_params, request_context, &self.subsystem)
                        .await?
                }
                ResolvedOperation::Pk(query) => {
                    query
                        .resolve(&mut validated_params, request_context, &self.subsystem)
                        .await?
                }
                ResolvedOperation::Unique(query) => {
                    query
                        .resolve(&mut validated_params, request_context, &self.subsystem)
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

    fn rpc_schema(&self) -> Option<&RpcSchema> {
        // TODO: We could just return &RpcSchema when all resolvers support schema
        Some(&self.rpc_schema)
    }
}

/// Find the matching query (PK or unique) based on which params are provided.
/// Returns an error if params don't match any known group.
fn find_matching_get_query<'a>(
    pk_query: Option<&'a PkQuery>,
    unique_queries: &[&'a UniqueQuery],
    provided_param_names: &[String],
) -> Result<ResolvedOperation<'a>, SubsystemRpcError> {
    let provided: HashSet<&str> = provided_param_names.iter().map(|s| s.as_str()).collect();

    // Check if provided params match the PK query's param names
    if let Some(pk) = pk_query {
        let pk_params = &pk.parameters.predicate_params;
        if provided.len() == pk_params.len() {
            let pk_param_names: HashSet<&str> = pk_params.iter().map(|p| p.name.as_str()).collect();
            if provided == pk_param_names {
                return Ok(ResolvedOperation::Pk(pk));
            }
        }
    }

    // Check if provided params match any unique query's param names
    for unique_query in unique_queries {
        if provided.len() == unique_query.parameters.predicate_params.len() {
            let unique_param_names: HashSet<&str> = unique_query
                .parameters
                .predicate_params
                .iter()
                .map(|p| p.name.as_str())
                .collect();
            if provided == unique_param_names {
                return Ok(ResolvedOperation::Unique(unique_query));
            }
        }
    }

    // Params were provided but don't match any known group
    let mut available_groups: Vec<String> = Vec::new();
    if let Some(pk) = pk_query {
        let mut names: Vec<&str> = pk
            .parameters
            .predicate_params
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        names.sort();
        available_groups.push(format!("pk({})", names.join(", ")));
    }
    for uq in unique_queries {
        let mut names: Vec<&str> = uq
            .parameters
            .predicate_params
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        names.sort();
        available_groups.push(format!("unique({})", names.join(", ")));
    }
    available_groups.sort();

    let mut sorted_provided: Vec<&str> = provided_param_names.iter().map(|s| s.as_str()).collect();
    sorted_provided.sort();

    Err(SubsystemRpcError::InvalidParams(format!(
        "Provided parameters [{}] do not match any lookup group. Available groups: {}",
        sorted_provided.join(", "),
        available_groups.join(", ")
    )))
}

/// Trait for resolving operations to an AbstractSelect.
/// Similar to GraphQL's OperationSelectionResolver pattern.
#[async_trait]
trait OperationSelectionResolver {
    async fn resolve_select<'a>(
        &'a self,
        validated_params: &mut HashMap<String, Val>,
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
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractOperation, SubsystemRpcError>;
}

/// Blanket implementation: any OperationSelectionResolver is also an OperationResolver
#[async_trait]
impl<T: OperationSelectionResolver + Send + Sync> OperationResolver for T {
    async fn resolve<'a>(
        &'a self,
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractOperation, SubsystemRpcError> {
        self.resolve_select(validated_params, request_context, subsystem)
            .await
            .map(AbstractOperation::Select)
    }
}

struct ComputeSelectOpts<'a> {
    predicate: AbstractPredicate,
    order_by: Option<AbstractOrderBy>,
    limit: Option<Limit>,
    offset: Option<Offset>,
    entity_type: &'a EntityType,
    return_type: &'a OperationReturnType<EntityType>,
}

/// Shared function to compute the final AbstractSelect.
/// Similar to GraphQL's compute_select pattern.
async fn compute_select<'a>(
    opts: ComputeSelectOpts<'a>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<AbstractSelect, SubsystemRpcError> {
    let ComputeSelectOpts {
        predicate,
        order_by,
        limit,
        offset,
        entity_type,
        return_type,
    } = opts;
    let selection_cardinality = match return_type {
        OperationReturnType::List(_) => SelectionCardinality::Many,
        _ => SelectionCardinality::One,
    };

    let mut elements = Vec::new();

    for field in &entity_type.fields {
        // Check field-level read access; only include fields that resolve to unconditional True.
        // Row-dependent predicates are skipped since RPC can't conditionally null per row.
        let field_access_predicate =
            compute_field_access_predicate(field, request_context, subsystem).await?;
        if field_access_predicate != AbstractPredicate::True {
            continue;
        }

        match &field.relation {
            PostgresRelation::Scalar { column_id, .. } => {
                elements.push(AliasedSelectionElement::new(
                    field.name.clone(),
                    SelectionElement::Physical(*column_id),
                ));
            }
            PostgresRelation::ManyToOne { relation, .. } => {
                let foreign_entity =
                    &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];

                let foreign_access_predicate =
                    compute_access_predicate(foreign_entity, request_context, subsystem).await?;

                // Select only PK scalar fields of the foreign entity
                let pk_elements: Vec<AliasedSelectionElement> = foreign_entity
                    .pk_fields()
                    .iter()
                    .filter_map(|pk_field| {
                        if let PostgresRelation::Scalar { column_id, .. } = pk_field.relation {
                            Some(AliasedSelectionElement::new(
                                pk_field.name.clone(),
                                SelectionElement::Physical(column_id),
                            ))
                        } else {
                            None
                        }
                    })
                    .collect();

                let nested_select = AbstractSelect {
                    table_id: foreign_entity.table_id,
                    selection: Selection::Json(pk_elements, SelectionCardinality::One),
                    predicate: foreign_access_predicate,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                elements.push(AliasedSelectionElement::new(
                    field.name.clone(),
                    SelectionElement::SubSelect(
                        RelationId::ManyToOne(relation.relation_id),
                        Box::new(nested_select),
                    ),
                ));
            }
            _ => {}
        }
    }

    let selection = Selection::Json(elements, selection_cardinality);

    Ok(AbstractSelect {
        table_id: entity_type.table_id,
        selection,
        predicate,
        order_by,
        offset,
        limit,
    })
}

/// Helper to compute field-level read access predicate
async fn compute_field_access_predicate<'a>(
    field: &PostgresField<EntityType>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<AbstractPredicate, SubsystemRpcError> {
    let access_expr = &subsystem.core_subsystem.database_access_expressions[field.access.read];

    subsystem
        .core_subsystem
        .solve(request_context, None, access_expr)
        .await
        .map_err(|_| SubsystemRpcError::Authorization)
        .map(|result| result.map(|p| p.0).resolve())
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
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractSelect, SubsystemRpcError> {
        let entity_type = self.return_type.typ(&subsystem.core_subsystem.entity_types);

        let access_predicate =
            compute_access_predicate(entity_type, request_context, subsystem).await?;

        // Extract the predicate parameter and compute the user predicate
        let user_predicate = match validated_params.remove(&self.parameters.predicate_param.name) {
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

        // Extract the order by parameter and compute the order by clause
        let order_by = match validated_params.remove(&self.parameters.order_by_param.name) {
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
        let limit =
            extract_i64_from_val(validated_params.remove(&self.parameters.limit_param.name))
                .map(Limit);
        let offset =
            extract_i64_from_val(validated_params.remove(&self.parameters.offset_param.name))
                .map(Offset);

        compute_select(
            ComputeSelectOpts {
                predicate,
                order_by,
                limit,
                offset,
                entity_type,
                return_type: &self.return_type,
            },
            request_context,
            subsystem,
        )
        .await
    }
}

/// Shared logic for resolving predicate params (used by both PK and unique queries)
async fn resolve_predicate_params<'a>(
    predicate_params: &[postgres_core_model::predicate::PredicateParameter],
    return_type: &OperationReturnType<EntityType>,
    validated_params: &mut HashMap<String, Val>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<AbstractSelect, SubsystemRpcError> {
    let entity_type = return_type.typ(&subsystem.core_subsystem.entity_types);

    let access_predicate =
        compute_access_predicate(entity_type, request_context, subsystem).await?;

    let mut predicates = Vec::new();
    for predicate_param in predicate_params {
        let param_value = validated_params
            .remove(&predicate_param.name)
            .ok_or_else(|| {
                SubsystemRpcError::InvalidParams(format!(
                    "Missing required parameter: {}",
                    predicate_param.name
                ))
            })?;

        let predicate = compute_predicate(
            predicate_param,
            &param_value,
            &subsystem.core_subsystem,
            request_context,
        )
        .await
        .map_err(from_postgres_error)?;

        predicates.push(predicate);
    }

    let query_predicate = predicates
        .into_iter()
        .reduce(AbstractPredicate::and)
        .unwrap_or(AbstractPredicate::True);

    let predicate = AbstractPredicate::and(query_predicate, access_predicate);

    compute_select(
        ComputeSelectOpts {
            predicate,
            order_by: None,
            limit: None,
            offset: None,
            entity_type,
            return_type,
        },
        request_context,
        subsystem,
    )
    .await
}

#[async_trait]
impl OperationSelectionResolver for PkQuery {
    async fn resolve_select<'a>(
        &'a self,
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractSelect, SubsystemRpcError> {
        resolve_predicate_params(
            &self.parameters.predicate_params,
            &self.return_type,
            validated_params,
            request_context,
            subsystem,
        )
        .await
    }
}

#[async_trait]
impl OperationSelectionResolver for UniqueQuery {
    async fn resolve_select<'a>(
        &'a self,
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractSelect, SubsystemRpcError> {
        resolve_predicate_params(
            &self.parameters.predicate_params,
            &self.return_type,
            validated_params,
            request_context,
            subsystem,
        )
        .await
    }
}

fn from_postgres_error(e: PostgresExecutionError) -> SubsystemRpcError {
    match e {
        PostgresExecutionError::Authorization => SubsystemRpcError::Authorization,
        _ => SubsystemRpcError::UserDisplayError(e.user_error_message()),
    }
}

/// Extract an i64 from a Val (for limit/offset).
fn extract_i64_from_val(val: Option<Val>) -> Option<i64> {
    match val {
        Some(Val::Number(n)) => n.as_i64(),
        _ => None,
    }
}
