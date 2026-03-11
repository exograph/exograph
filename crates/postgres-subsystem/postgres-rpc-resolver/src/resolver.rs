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
    AbstractDelete, AbstractOperation, AbstractOrderBy, AbstractPredicate, AbstractSelect,
    AliasedSelectionElement, DatabaseExecutor, Limit, Offset, RelationId, Selection,
    SelectionCardinality, SelectionElement,
};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_model::types::EntityType;
use postgres_core_resolver::database_helper::extractor;
use postgres_core_resolver::order_by_mapper::compute_order_by;
use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;
use postgres_core_resolver::predicate_mapper::compute_predicate;
use postgres_rpc_model::operation::{
    CollectionDelete, CollectionQuery, HasPredicateParams, PkDelete, PkQuery, UniqueDelete,
    UniqueQuery,
};
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

        // Try to find the matching operation: collection query/delete, then PK/unique query/delete
        let resolved: Option<&dyn OperationResolver> = self
            .subsystem
            .collection_queries
            .get_by_key(request_method)
            .map(|q| q as &dyn OperationResolver)
            .or_else(|| {
                self.subsystem
                    .collection_deletes
                    .get_by_key(request_method)
                    .map(|d| d as &dyn OperationResolver)
            });

        // For PK/unique lookups, unwrap `by` once and search across both queries and deletes
        let resolved = if let Some(r) = resolved {
            Some(r)
        } else {
            resolve_by_lookup(
                self.subsystem.pk_queries.get_by_key(request_method),
                self.subsystem
                    .unique_queries
                    .iter()
                    .filter(|(_, q)| q.name == request_method)
                    .map(|(_, q)| q)
                    .collect(),
                self.subsystem.pk_deletes.get_by_key(request_method),
                self.subsystem
                    .unique_deletes
                    .iter()
                    .filter(|(_, d)| d.name == request_method)
                    .map(|(_, d)| d)
                    .collect(),
                &mut validated_params,
            )?
        };

        if let Some(resolver) = resolved {
            let operation = resolver
                .resolve(&mut validated_params, request_context, &self.subsystem)
                .await?;

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

/// Unwrap the `by` param and find the matching PK or unique operation across queries and deletes.
fn resolve_by_lookup<'a>(
    pk_query: Option<&'a (impl HasPredicateParams + OperationResolver)>,
    unique_queries: Vec<&'a (impl HasPredicateParams + OperationResolver)>,
    pk_delete: Option<&'a (impl HasPredicateParams + OperationResolver)>,
    unique_deletes: Vec<&'a (impl HasPredicateParams + OperationResolver)>,
    validated_params: &mut HashMap<String, Val>,
) -> Result<Option<&'a dyn OperationResolver>, SubsystemRpcError> {
    if pk_query.is_none()
        && unique_queries.is_empty()
        && pk_delete.is_none()
        && unique_deletes.is_empty()
    {
        return Ok(None);
    }

    // Extract the `by` param once
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

    let provided: HashSet<&str> = by_fields.keys().map(|s| s.as_str()).collect();

    // Try to match queries first, then deletes
    let matched: Option<&dyn OperationResolver> = None;

    let matched = matched.or_else(|| {
        if let Some(pk) = pk_query
            && provided == param_name_set(pk.predicate_params())
        {
            return Some(pk as &dyn OperationResolver);
        }
        unique_queries
            .iter()
            .find(|op| provided == param_name_set(op.predicate_params()))
            .map(|op| *op as &dyn OperationResolver)
    });

    let matched = matched.or_else(|| {
        if let Some(pk) = pk_delete
            && provided == param_name_set(pk.predicate_params())
        {
            return Some(pk as &dyn OperationResolver);
        }
        unique_deletes
            .iter()
            .find(|op| provided == param_name_set(op.predicate_params()))
            .map(|op| *op as &dyn OperationResolver)
    });

    if let Some(op) = matched {
        *validated_params = by_fields;
        return Ok(Some(op));
    }

    // Build error message with available groups from all op sources
    let mut available_groups: Vec<String> = Vec::new();
    if let Some(pk) = pk_query {
        available_groups.push(format!("pk({})", sorted_param_names(pk.predicate_params())));
    }
    for uq in &unique_queries {
        available_groups.push(format!(
            "unique({})",
            sorted_param_names(uq.predicate_params())
        ));
    }
    if let Some(pk) = pk_delete {
        available_groups.push(format!("pk({})", sorted_param_names(pk.predicate_params())));
    }
    for ud in &unique_deletes {
        available_groups.push(format!(
            "unique({})",
            sorted_param_names(ud.predicate_params())
        ));
    }
    available_groups.sort();
    available_groups.dedup();

    let mut sorted_provided: Vec<&str> = provided.into_iter().collect();
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
            solve_access_expression(field.access.read, request_context, subsystem).await?;
        // TODO: Make this less strict. We will first need to make schema make access controlled fields nullable,
        // then we can include fields with row-dependent access predicates.
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

                let foreign_access_predicate = compute_entity_access_predicate(
                    foreign_entity,
                    AccessKind::Read,
                    request_context,
                    subsystem,
                )
                .await?;

                // Select only PK scalar fields of the foreign entity that are accessible
                // TODO: We should warn users during model building that adding access control to pk field will have implications
                let mut pk_elements = Vec::new();
                for pk_field in foreign_entity.pk_fields() {
                    let column_id = match pk_field.relation {
                        PostgresRelation::Scalar { column_id, .. } => column_id,
                        _ => continue,
                    };

                    let pk_field_access =
                        solve_access_expression(pk_field.access.read, request_context, subsystem)
                            .await?;
                    if pk_field_access != AbstractPredicate::True {
                        continue;
                    }

                    pk_elements.push(AliasedSelectionElement::new(
                        pk_field.name.clone(),
                        SelectionElement::Physical(column_id),
                    ));
                }

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

async fn solve_access_expression<'a>(
    access_expr_index: core_model::mapped_arena::SerializableSlabIndex<
        core_model::access::AccessPredicateExpression<
            postgres_core_model::access::DatabaseAccessPrimitiveExpression,
        >,
    >,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<AbstractPredicate, SubsystemRpcError> {
    let access_expr = &subsystem.core_subsystem.database_access_expressions[access_expr_index];

    subsystem
        .core_subsystem
        .solve(request_context, None, access_expr)
        .await
        .map_err(|_| SubsystemRpcError::Authorization)
        .map(|result| result.map(|p| p.0).resolve())
}

/// Compute an entity-level access predicate.
/// For delete, a full `False` predicate returns an explicit authorization error.
async fn compute_entity_access_predicate<'a>(
    entity_type: &EntityType,
    access_kind: AccessKind,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<AbstractPredicate, SubsystemRpcError> {
    let access_index = match access_kind {
        AccessKind::Read => entity_type.access.read,
        AccessKind::Delete => entity_type.access.delete,
    };

    let predicate = solve_access_expression(access_index, request_context, subsystem).await?;

    if matches!(access_kind, AccessKind::Delete) && predicate == AbstractPredicate::False {
        return Err(SubsystemRpcError::Authorization);
    }

    Ok(predicate)
}

enum AccessKind {
    Read,
    Delete,
}

/// Build a PK-only AbstractSelect for the RETURNING clause of delete operations.
/// Only selects PK columns — no read access checks needed since the user proved delete access.
fn compute_pk_only_select(
    entity_type: &EntityType,
    return_type: &OperationReturnType<EntityType>,
) -> AbstractSelect {
    let selection_cardinality = match return_type {
        OperationReturnType::List(_) => SelectionCardinality::Many,
        _ => SelectionCardinality::One,
    };

    let elements: Vec<AliasedSelectionElement> = entity_type
        .pk_fields()
        .iter()
        .filter_map(|field| match &field.relation {
            PostgresRelation::Scalar { column_id, .. } => Some(AliasedSelectionElement::new(
                field.name.clone(),
                SelectionElement::Physical(*column_id),
            )),
            _ => None,
        })
        .collect();

    let selection = Selection::Json(elements, selection_cardinality);

    AbstractSelect {
        table_id: entity_type.table_id,
        selection,
        predicate: AbstractPredicate::True,
        order_by: None,
        offset: None,
        limit: None,
    }
}

/// Shared logic for resolving delete predicate params (PK or unique delete)
async fn resolve_delete_predicate_params<'a>(
    predicate_params: &[postgres_core_model::predicate::PredicateParameter],
    return_type: &OperationReturnType<EntityType>,
    validated_params: &mut HashMap<String, Val>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<AbstractOperation, SubsystemRpcError> {
    let entity_type = return_type.typ(&subsystem.core_subsystem.entity_types);

    let access_predicate = compute_entity_access_predicate(
        entity_type,
        AccessKind::Delete,
        request_context,
        subsystem,
    )
    .await?;

    let query_predicate = resolve_predicate_param_list(
        predicate_params,
        validated_params,
        request_context,
        subsystem,
    )
    .await?;

    let predicate = AbstractPredicate::and(query_predicate, access_predicate);

    let selection = compute_pk_only_select(entity_type, return_type);

    Ok(AbstractOperation::Delete(AbstractDelete {
        table_id: entity_type.table_id,
        predicate,
        selection,
        precheck_predicates: vec![AbstractPredicate::True],
    }))
}

macro_rules! impl_delete_resolver {
    ($($ty:ty),+) => { $(
        #[async_trait]
        impl OperationResolver for $ty {
            async fn resolve<'a>(
                &'a self,
                validated_params: &mut HashMap<String, Val>,
                request_context: &'a RequestContext<'a>,
                subsystem: &'a PostgresRpcSubsystemWithRouter,
            ) -> Result<AbstractOperation, SubsystemRpcError> {
                resolve_delete_predicate_params(
                    &self.parameters.predicate_params,
                    &self.return_type,
                    validated_params,
                    request_context,
                    subsystem,
                )
                .await
            }
        }
    )+ };
}

impl_delete_resolver!(PkDelete, UniqueDelete);

#[async_trait]
impl OperationResolver for CollectionDelete {
    async fn resolve<'a>(
        &'a self,
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<AbstractOperation, SubsystemRpcError> {
        let entity_type = self.return_type.typ(&subsystem.core_subsystem.entity_types);

        let access_predicate = compute_entity_access_predicate(
            entity_type,
            AccessKind::Delete,
            request_context,
            subsystem,
        )
        .await?;

        let user_predicate = resolve_optional_predicate_param(
            &self.parameters.predicate_param,
            validated_params,
            request_context,
            subsystem,
        )
        .await?;

        let predicate = AbstractPredicate::and(user_predicate, access_predicate);

        let selection = compute_pk_only_select(entity_type, &self.return_type);

        Ok(AbstractOperation::Delete(AbstractDelete {
            table_id: entity_type.table_id,
            predicate,
            selection,
            precheck_predicates: vec![AbstractPredicate::True],
        }))
    }
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

        let access_predicate = compute_entity_access_predicate(
            entity_type,
            AccessKind::Read,
            request_context,
            subsystem,
        )
        .await?;

        let user_predicate = resolve_optional_predicate_param(
            &self.parameters.predicate_param,
            validated_params,
            request_context,
            subsystem,
        )
        .await?;

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

/// Resolve an optional predicate param (for `where` clauses in collection operations).
async fn resolve_optional_predicate_param<'a>(
    param: &postgres_core_model::predicate::PredicateParameter,
    validated_params: &mut HashMap<String, Val>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<AbstractPredicate, SubsystemRpcError> {
    match validated_params.remove(&param.name) {
        Some(val) => compute_predicate(param, &val, &subsystem.core_subsystem, request_context)
            .await
            .map_err(from_postgres_error),
        None => Ok(AbstractPredicate::True),
    }
}

/// Shared logic for resolving a list of predicate params into a combined predicate.
/// Used by both PK/unique queries and PK/unique deletes.
async fn resolve_predicate_param_list<'a>(
    predicate_params: &[postgres_core_model::predicate::PredicateParameter],
    validated_params: &mut HashMap<String, Val>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<AbstractPredicate, SubsystemRpcError> {
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

    Ok(predicates
        .into_iter()
        .reduce(AbstractPredicate::and)
        .unwrap_or(AbstractPredicate::True))
}

/// Shared logic for resolving PK/unique query predicate params into a select
async fn resolve_query_predicate_params<'a>(
    predicate_params: &[postgres_core_model::predicate::PredicateParameter],
    return_type: &OperationReturnType<EntityType>,
    validated_params: &mut HashMap<String, Val>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<AbstractSelect, SubsystemRpcError> {
    let entity_type = return_type.typ(&subsystem.core_subsystem.entity_types);

    let access_predicate =
        compute_entity_access_predicate(entity_type, AccessKind::Read, request_context, subsystem)
            .await?;

    let query_predicate = resolve_predicate_param_list(
        predicate_params,
        validated_params,
        request_context,
        subsystem,
    )
    .await?;

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

macro_rules! impl_query_selection_resolver {
    ($($ty:ty),+) => { $(
        #[async_trait]
        impl OperationSelectionResolver for $ty {
            async fn resolve_select<'a>(
                &'a self,
                validated_params: &mut HashMap<String, Val>,
                request_context: &'a RequestContext<'a>,
                subsystem: &'a PostgresRpcSubsystemWithRouter,
            ) -> Result<AbstractSelect, SubsystemRpcError> {
                resolve_query_predicate_params(
                    &self.parameters.predicate_params,
                    &self.return_type,
                    validated_params,
                    request_context,
                    subsystem,
                )
                .await
            }
        }
    )+ };
}

impl_query_selection_resolver!(PkQuery, UniqueQuery);

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

fn param_name_set(params: &[postgres_core_model::predicate::PredicateParameter]) -> HashSet<&str> {
    params.iter().map(|p| p.name.as_str()).collect()
}

fn sorted_param_names(params: &[postgres_core_model::predicate::PredicateParameter]) -> String {
    let mut names: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    names.join(", ")
}
