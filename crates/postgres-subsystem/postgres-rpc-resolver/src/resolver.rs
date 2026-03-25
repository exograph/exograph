// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use common::context::RequestContext;
use common::value::Val;

use core_model::access::AccessPredicateExpression;
use core_model::mapped_arena::SerializableSlabIndex;
use core_model::types::OperationReturnType;
use core_resolver::access_solver::{AccessInput, AccessSolver};
use core_resolver::plugin::SubsystemRpcResolver;
use core_resolver::plugin::subsystem_rpc_resolver::{SubsystemRpcError, SubsystemRpcResponse};
use core_resolver::{QueryResponse, QueryResponseBody};
use exo_sql::{
    AbstractDelete, AbstractInsert, AbstractOperation, AbstractPredicate, AbstractSelect,
    AbstractUpdate, AliasedSelectionElement, Column, ColumnId, ColumnPath, ColumnValuePair,
    DatabaseExecutor, InsertionElement, InsertionRow, Limit, ManyToOne, NestedAbstractDelete,
    NestedAbstractInsert, NestedAbstractInsertSet, NestedAbstractUpdate, NestedInsertion, Offset,
    OneToMany, PgAbstractOperation, PgAbstractOrderBy, PgAbstractPredicate, PgAbstractSelect,
    PgAliasedSelectionElement, PgInsertionElement, PgInsertionRow, PgNestedAbstractDelete,
    PgNestedAbstractInsertSet, PgNestedAbstractUpdate, PhysicalColumnPath, RelationId, Selection,
    SelectionCardinality, SelectionElement,
};
use postgres_core_model::access::{
    DatabaseAccessPrimitiveExpression, PrecheckAccessPrimitiveExpression,
};
use postgres_core_model::relation::ManyToOneRelation;
use postgres_core_model::relation::OneToManyRelation;
use postgres_core_model::relation::PostgresRelation;
use postgres_core_model::types::{EntityType, PostgresField};
use postgres_core_resolver::cast;
use postgres_core_resolver::database_helper::extractor;
use postgres_core_resolver::order_by_mapper::compute_order_by;
use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;
use postgres_core_resolver::predicate_mapper::compute_predicate;
use postgres_core_resolver::predicate_util::get_argument_field;
use postgres_rpc_model::operation::{
    CollectionDelete, CollectionQuery, CollectionUpdate, Create, PkDelete, PkQuery, PkUpdate,
    UniqueDelete, UniqueQuery, UniqueUpdate,
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

        // Try to find the matching operation by method name (queries before deletes)
        let resolved: Option<&dyn OperationResolver> = self
            .subsystem
            .collection_queries
            .get_by_key(request_method)
            .map(|q| q as &dyn OperationResolver)
            .or_else(|| {
                self.subsystem
                    .pk_queries
                    .get_by_key(request_method)
                    .map(|q| q as &dyn OperationResolver)
            })
            .or_else(|| {
                self.subsystem
                    .unique_queries
                    .get_by_key(request_method)
                    .map(|q| q as &dyn OperationResolver)
            })
            .or_else(|| {
                self.subsystem
                    .collection_deletes
                    .get_by_key(request_method)
                    .map(|d| d as &dyn OperationResolver)
            })
            .or_else(|| {
                self.subsystem
                    .pk_deletes
                    .get_by_key(request_method)
                    .map(|d| d as &dyn OperationResolver)
            })
            .or_else(|| {
                self.subsystem
                    .unique_deletes
                    .get_by_key(request_method)
                    .map(|d| d as &dyn OperationResolver)
            })
            .or_else(|| {
                self.subsystem
                    .collection_updates
                    .get_by_key(request_method)
                    .map(|u| u as &dyn OperationResolver)
            })
            .or_else(|| {
                self.subsystem
                    .pk_updates
                    .get_by_key(request_method)
                    .map(|u| u as &dyn OperationResolver)
            })
            .or_else(|| {
                self.subsystem
                    .unique_updates
                    .get_by_key(request_method)
                    .map(|u| u as &dyn OperationResolver)
            })
            .or_else(|| {
                self.subsystem
                    .creates
                    .get_by_key(request_method)
                    .map(|c| c as &dyn OperationResolver)
            })
            .or_else(|| {
                self.subsystem
                    .collection_creates
                    .get_by_key(request_method)
                    .map(|c| c as &dyn OperationResolver)
            });

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

/// Trait for resolving operations to a PgAbstractSelect.
/// Similar to GraphQL's OperationSelectionResolver pattern.
#[async_trait]
trait OperationSelectionResolver {
    async fn resolve_select<'a>(
        &'a self,
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<PgAbstractSelect, SubsystemRpcError>;
}

/// Trait for resolving operations to a PgAbstractOperation.
/// Blanket impl wraps PgAbstractSelect in AbstractOperation::Select.
#[async_trait]
trait OperationResolver {
    async fn resolve<'a>(
        &'a self,
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<PgAbstractOperation, SubsystemRpcError>;
}

/// Blanket implementation: any OperationSelectionResolver is also an OperationResolver
#[async_trait]
impl<T: OperationSelectionResolver + Send + Sync> OperationResolver for T {
    async fn resolve<'a>(
        &'a self,
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<PgAbstractOperation, SubsystemRpcError> {
        self.resolve_select(validated_params, request_context, subsystem)
            .await
            .map(AbstractOperation::Select)
    }
}

struct ComputeSelectOpts<'a> {
    predicate: PgAbstractPredicate,
    order_by: Option<PgAbstractOrderBy>,
    limit: Option<Limit>,
    offset: Option<Offset>,
    entity_type: &'a EntityType,
    return_type: &'a OperationReturnType<EntityType>,
}

/// Shared function to compute the final PgAbstractSelect.
/// Similar to GraphQL's compute_select pattern.
async fn compute_select<'a>(
    opts: ComputeSelectOpts<'a>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<PgAbstractSelect, SubsystemRpcError> {
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

async fn solve_access<'a>(
    access_expr: &AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<PgAbstractPredicate, SubsystemRpcError> {
    subsystem
        .core_subsystem
        .solve(request_context, None, access_expr)
        .await
        .map_err(|_| SubsystemRpcError::Authorization)
        .map(|result| result.map(|p| p.0).resolve())
}

async fn solve_access_expression<'a>(
    access_expr_index: SerializableSlabIndex<
        AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    >,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<PgAbstractPredicate, SubsystemRpcError> {
    let access_expr = &subsystem.core_subsystem.database_access_expressions[access_expr_index];
    solve_access(access_expr, request_context, subsystem).await
}

/// Compute an entity-level access predicate.
/// For delete, a full `False` predicate returns an explicit authorization error.
async fn compute_entity_access_predicate<'a>(
    entity_type: &EntityType,
    access_kind: AccessKind,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<PgAbstractPredicate, SubsystemRpcError> {
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

/// Compute create access predicates (entity-level and field-level precheck).
/// Returns the precheck predicate for the insert operation.
async fn compute_create_access<'a>(
    entity_type: &EntityType,
    data_val: &'a Val,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<PgAbstractPredicate, SubsystemRpcError> {
    let access_input = AccessInput {
        value: data_val,
        ignore_missing_value: true,
        aliases: HashMap::new(),
    };

    // Entity-level precheck
    let precheck_predicate = subsystem
        .core_subsystem
        .solve(
            request_context,
            Some(&access_input),
            &subsystem.core_subsystem.precheck_expressions[entity_type.access.creation.precheck],
        )
        .await
        .map_err(|_| SubsystemRpcError::Authorization)?
        .map(|p| p.0)
        .resolve();

    if precheck_predicate == AbstractPredicate::False {
        return Err(SubsystemRpcError::Authorization);
    }

    // Field-level precheck for creation
    let field_precheck = compute_field_precheck(
        entity_type,
        data_val,
        &access_input,
        request_context,
        subsystem,
        |f| f.access.creation.precheck,
    )
    .await?;

    if field_precheck == AbstractPredicate::False {
        return Err(SubsystemRpcError::Authorization);
    }

    Ok(AbstractPredicate::and(precheck_predicate, field_precheck))
}

/// Compute field-level precheck predicates for the given data.
/// `get_precheck_index` selects which precheck expression to use for each field
/// (e.g., creation vs update).
async fn compute_field_precheck<'a, F>(
    entity_type: &EntityType,
    data_val: &'a Val,
    access_input: &AccessInput<'a>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
    get_precheck_index: F,
) -> Result<PgAbstractPredicate, SubsystemRpcError>
where
    F: Fn(
        &PostgresField<EntityType>,
    )
        -> SerializableSlabIndex<AccessPredicateExpression<PrecheckAccessPrimitiveExpression>>,
{
    let data_fields = match data_val {
        Val::Object(fields) => fields,
        _ => return Ok(AbstractPredicate::True),
    };

    let mut combined = AbstractPredicate::True;

    // For field-level access checks, use ignore_missing_value: false to match GraphQL behavior.
    // The entity-level precheck uses true (update data may omit fields), but field-level checks
    // must not ignore missing values — otherwise expressions like `self.authId == AuthContext.id`
    // would resolve to True when `authId` is absent from the input, bypassing the check.
    let field_access_input = AccessInput {
        value: access_input.value,
        ignore_missing_value: false,
        aliases: access_input.aliases.clone(),
    };

    for field_name in data_fields.keys().map(|k| k.as_str()) {
        if let Some(field) = entity_type.field_by_name(field_name) {
            let field_predicate = subsystem
                .core_subsystem
                .solve(
                    request_context,
                    Some(&field_access_input),
                    &subsystem.core_subsystem.precheck_expressions[get_precheck_index(field)],
                )
                .await
                .map_err(|_| SubsystemRpcError::Authorization)?
                .map(|p| p.0)
                .resolve();

            if field_predicate == AbstractPredicate::False {
                return Err(SubsystemRpcError::Authorization);
            }

            combined = AbstractPredicate::and(combined, field_predicate);
        }
    }

    Ok(combined)
}

/// Compute column values from the `data` parameter for a create operation.
/// `parent_entity` is the parent entity for nested creates (used to skip back-references).
fn compute_create_columns<'a>(
    entity_type: &'a EntityType,
    data_val: &'a Val,
    parent_entity: Option<&'a EntityType>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> futures::future::BoxFuture<'a, Result<Vec<PgInsertionElement>, SubsystemRpcError>> {
    Box::pin(compute_create_columns_inner(
        entity_type,
        data_val,
        parent_entity,
        request_context,
        subsystem,
    ))
}

/// Check if a field is a ManyToOne back-reference to the parent entity.
fn is_back_reference(
    field: &PostgresField<EntityType>,
    parent_entity: Option<&EntityType>,
    subsystem: &PostgresRpcSubsystemWithRouter,
) -> bool {
    if let (Some(parent), PostgresRelation::ManyToOne { relation, .. }) =
        (parent_entity, &field.relation)
    {
        let target_entity = &subsystem.core_subsystem.entity_types[relation.foreign_entity_id];
        target_entity.table_id == parent.table_id
    } else {
        false
    }
}

async fn compute_create_columns_inner<'a>(
    entity_type: &'a EntityType,
    data_val: &'a Val,
    parent_entity: Option<&'a EntityType>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<Vec<PgInsertionElement>, SubsystemRpcError> {
    let mut elements = Vec::new();

    for field in &entity_type.fields {
        // Skip ManyToOne fields that reference back to the parent entity
        if is_back_reference(field, parent_entity, subsystem) {
            continue;
        }

        match &field.relation {
            PostgresRelation::Scalar { column_id, .. } => {
                if let Some(value) = get_argument_field(data_val, &field.name) {
                    let column = column_id.get_column(&subsystem.core_subsystem.database);
                    let value_column = cast::literal_column(value, column)
                        .map_err(|e| SubsystemRpcError::UserDisplayError(e.user_error_message()))?;
                    elements.push(InsertionElement::SelfInsert(ColumnValuePair {
                        column: *column_id,
                        value: value_column,
                    }));
                }
            }
            PostgresRelation::ManyToOne {
                relation:
                    ManyToOneRelation {
                        foreign_pk_field_ids,
                        relation_id,
                        ..
                    },
                ..
            } => {
                let ManyToOne { column_pairs, .. } =
                    relation_id.deref(&subsystem.core_subsystem.database);

                match get_argument_field(data_val, &field.name) {
                    Some(Val::Null) => {
                        for column_pair in column_pairs.iter() {
                            elements.push(InsertionElement::SelfInsert(ColumnValuePair {
                                column: column_pair.self_column_id,
                                value: Column::Null,
                            }));
                        }
                    }
                    Some(argument_value) => {
                        for (column_pair, foreign_pk_field_id) in
                            column_pairs.iter().zip(foreign_pk_field_ids.iter())
                        {
                            let self_column_id = column_pair.self_column_id;
                            let self_column =
                                self_column_id.get_column(&subsystem.core_subsystem.database);
                            let foreign_type_pk_field_name = &foreign_pk_field_id
                                .resolve(&subsystem.core_subsystem.entity_types)
                                .name;

                            if let Some(foreign_type_pk_arg) =
                                get_argument_field(argument_value, foreign_type_pk_field_name)
                            {
                                let value_column =
                                    cast::literal_column(foreign_type_pk_arg, self_column)
                                        .map_err(|e| {
                                            SubsystemRpcError::UserDisplayError(
                                                e.user_error_message(),
                                            )
                                        })?;
                                elements.push(InsertionElement::SelfInsert(ColumnValuePair {
                                    column: self_column_id,
                                    value: value_column,
                                }));
                            }
                        }
                    }
                    None => {}
                }
            }
            PostgresRelation::OneToMany(one_to_many_relation) => {
                if let Some(nested_val) = get_argument_field(data_val, &field.name) {
                    let foreign_entity = &subsystem.core_subsystem.entity_types
                        [one_to_many_relation.foreign_entity_id];
                    let (insertions, precheck_predicates) = build_nested_insertions(
                        foreign_entity,
                        Some(entity_type),
                        nested_val,
                        request_context,
                        subsystem,
                    )
                    .await?;
                    elements.push(InsertionElement::NestedInsert(NestedInsertion {
                        relation_id: one_to_many_relation.relation_id,
                        insertions,
                        precheck_predicates,
                    }));
                }
            }
            _ => {}
        }
    }

    Ok(elements)
}

type NestedInsertionsResult =
    Result<(Vec<PgInsertionRow>, Vec<PgAbstractPredicate>), SubsystemRpcError>;

/// Build nested insertions from a Val (list or single object) for OneToMany creates.
fn build_nested_insertions<'a>(
    foreign_entity: &'a EntityType,
    parent_entity: Option<&'a EntityType>,
    nested_val: &'a Val,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> futures::future::BoxFuture<'a, NestedInsertionsResult> {
    Box::pin(async move {
        let items = val_as_items(nested_val)?;

        let mut rows = Vec::new();
        let mut precheck_predicates = Vec::new();

        for item in items {
            let precheck =
                compute_create_access(foreign_entity, item, request_context, subsystem).await?;
            let elements = compute_create_columns(
                foreign_entity,
                item,
                parent_entity,
                request_context,
                subsystem,
            )
            .await?;
            rows.push(InsertionRow { elems: elements });
            precheck_predicates.push(precheck);
        }

        Ok((rows, precheck_predicates))
    })
}

/// Build an insert operation from a single data value.
async fn build_single_insert<'a>(
    entity_type: &EntityType,
    data_val: Val,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<(PgInsertionRow, PgAbstractPredicate), SubsystemRpcError> {
    let precheck_predicate =
        compute_create_access(entity_type, &data_val, request_context, subsystem).await?;

    let elements =
        compute_create_columns(entity_type, &data_val, None, request_context, subsystem).await?;

    Ok((InsertionRow { elems: elements }, precheck_predicate))
}

/// Build a create AbstractInsert<PgExtension> operation.
async fn build_create_operation<'a>(
    entity_type: &EntityType,
    return_type: &OperationReturnType<EntityType>,
    data_val: Val,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<PgAbstractOperation, SubsystemRpcError> {
    let selection = compute_pk_only_select(entity_type, return_type);

    let (rows, precheck_predicates) = match data_val {
        Val::List(items) => {
            let results =
                futures::future::try_join_all(items.into_iter().map(|item| {
                    build_single_insert(entity_type, item, request_context, subsystem)
                }))
                .await?;
            results.into_iter().unzip()
        }
        _ => {
            let (row, precheck) =
                build_single_insert(entity_type, data_val, request_context, subsystem).await?;
            (vec![row], vec![precheck])
        }
    };

    Ok(AbstractOperation::Insert(AbstractInsert {
        table_id: entity_type.table_id,
        rows,
        selection,
        precheck_predicates,
    }))
}

#[async_trait]
impl OperationResolver for Create {
    async fn resolve<'a>(
        &'a self,
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<PgAbstractOperation, SubsystemRpcError> {
        let entity_type = self.return_type.typ(&subsystem.core_subsystem.entity_types);
        let data_param_name = &self.parameters.data_param.name;

        let data_val = validated_params
            .remove(data_param_name.as_str())
            .ok_or_else(|| {
                SubsystemRpcError::InvalidParams(format!(
                    "Missing required parameter: {data_param_name}"
                ))
            })?;

        build_create_operation(
            entity_type,
            &self.return_type,
            data_val,
            request_context,
            subsystem,
        )
        .await
    }
}

/// Build a PK-only PgAbstractSelect for the RETURNING clause of delete operations.
/// Only selects PK columns — no read access checks needed since the user proved delete access.
fn compute_pk_only_select(
    entity_type: &EntityType,
    return_type: &OperationReturnType<EntityType>,
) -> PgAbstractSelect {
    let selection_cardinality = match return_type {
        OperationReturnType::List(_) => SelectionCardinality::Many,
        _ => SelectionCardinality::One,
    };

    let elements: Vec<PgAliasedSelectionElement> = entity_type
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
) -> Result<PgAbstractOperation, SubsystemRpcError> {
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
            ) -> Result<PgAbstractOperation, SubsystemRpcError> {
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
    ) -> Result<PgAbstractOperation, SubsystemRpcError> {
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

/// Compute the update access predicates (both precheck and database).
/// Returns (precheck_predicate, database_predicate).
async fn compute_update_access<'a>(
    entity_type: &EntityType,
    data_val: &'a Val,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<(PgAbstractPredicate, PgAbstractPredicate), SubsystemRpcError> {
    let access_input = AccessInput {
        value: data_val,
        ignore_missing_value: true,
        aliases: HashMap::new(),
    };

    // Entity-level precheck (validates new data against access rules)
    let precheck_predicate = subsystem
        .core_subsystem
        .solve(
            request_context,
            Some(&access_input),
            &subsystem.core_subsystem.precheck_expressions[entity_type.access.update.precheck],
        )
        .await
        .map_err(|_| SubsystemRpcError::Authorization)?
        .map(|p| p.0)
        .resolve();

    if precheck_predicate == AbstractPredicate::False {
        return Err(SubsystemRpcError::Authorization);
    }

    // Field-level precheck (validates each provided field against its access rules)
    let field_precheck = compute_field_precheck(
        entity_type,
        data_val,
        &access_input,
        request_context,
        subsystem,
        |f| f.access.update.precheck,
    )
    .await?;

    if field_precheck == AbstractPredicate::False {
        return Err(SubsystemRpcError::Authorization);
    }

    // Database-level access predicate (the WHERE clause restriction)
    let database_predicate = solve_access_expression(
        entity_type.access.update.database,
        request_context,
        subsystem,
    )
    .await?;

    if database_predicate == AbstractPredicate::False {
        return Err(SubsystemRpcError::Authorization);
    }

    // Field-level precheck goes into database predicate (WHERE clause), not into precheck
    // assertions, matching the GraphQL behavior. The precheck assertion mechanism expects
    // exactly 1 row, but field-level predicates with relation traversals can produce joins
    // that return multiple rows.
    let combined_database_predicate = AbstractPredicate::and(database_predicate, field_precheck);

    Ok((precheck_predicate, combined_database_predicate))
}

/// Extract the `data` parameter, compute access, and build the AbstractUpdate<PgExtension>.
async fn build_update_operation<'a>(
    entity_type: &EntityType,
    return_type: &OperationReturnType<EntityType>,
    data_val: Val,
    query_predicate: PgAbstractPredicate,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<PgAbstractOperation, SubsystemRpcError> {
    let (precheck_predicate, access_predicate) =
        compute_update_access(entity_type, &data_val, request_context, subsystem).await?;

    let predicate = AbstractPredicate::and(query_predicate, access_predicate);

    let column_values = compute_update_columns(entity_type, &data_val, subsystem)?;
    let selection = compute_pk_only_select(entity_type, return_type);

    let (nested_updates, nested_inserts, nested_deletes) =
        compute_nested_update_ops(entity_type, &data_val, request_context, subsystem).await?;

    Ok(AbstractOperation::Update(AbstractUpdate {
        table_id: entity_type.table_id,
        predicate,
        column_values,
        selection,
        nested_updates,
        nested_inserts,
        nested_deletes,
        precheck_predicates: vec![precheck_predicate],
    }))
}

/// Shared logic for resolving update predicate params (PK or unique update)
async fn resolve_update_predicate_params<'a>(
    predicate_params: &[postgres_core_model::predicate::PredicateParameter],
    data_param_name: &str,
    return_type: &OperationReturnType<EntityType>,
    validated_params: &mut HashMap<String, Val>,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<PgAbstractOperation, SubsystemRpcError> {
    let entity_type = return_type.typ(&subsystem.core_subsystem.entity_types);

    let data_val = validated_params.remove(data_param_name).ok_or_else(|| {
        SubsystemRpcError::InvalidParams(format!("Missing required parameter: {data_param_name}"))
    })?;

    let query_predicate = resolve_predicate_param_list(
        predicate_params,
        validated_params,
        request_context,
        subsystem,
    )
    .await?;

    build_update_operation(
        entity_type,
        return_type,
        data_val,
        query_predicate,
        request_context,
        subsystem,
    )
    .await
}

/// Compute column values from the `data` parameter for an update operation.
fn compute_update_columns(
    entity_type: &EntityType,
    data_val: &Val,
    subsystem: &PostgresRpcSubsystemWithRouter,
) -> Result<Vec<(ColumnId, Column)>, SubsystemRpcError> {
    let mut column_values = Vec::new();

    for field in &entity_type.fields {
        // Skip PK fields - they shouldn't be updated
        if field.relation.is_pk() {
            continue;
        }

        match &field.relation {
            PostgresRelation::Scalar { column_id, .. } => {
                if let Some(value) = get_argument_field(data_val, &field.name) {
                    let column = column_id.get_column(&subsystem.core_subsystem.database);
                    let value_column = cast::literal_column(value, column)
                        .map_err(|e| SubsystemRpcError::UserDisplayError(e.user_error_message()))?;
                    column_values.push((*column_id, value_column));
                }
            }
            PostgresRelation::ManyToOne {
                relation:
                    ManyToOneRelation {
                        foreign_pk_field_ids,
                        relation_id,
                        ..
                    },
                ..
            } => {
                let ManyToOne { column_pairs, .. } =
                    relation_id.deref(&subsystem.core_subsystem.database);

                // Check for the field value once, outside the column_pairs loop
                match get_argument_field(data_val, &field.name) {
                    Some(Val::Null) => {
                        for column_pair in column_pairs.iter() {
                            column_values.push((column_pair.self_column_id, Column::Null));
                        }
                    }
                    Some(argument_value) => {
                        for (column_pair, foreign_pk_field_id) in
                            column_pairs.iter().zip(foreign_pk_field_ids.iter())
                        {
                            let self_column_id = column_pair.self_column_id;
                            let self_column =
                                self_column_id.get_column(&subsystem.core_subsystem.database);
                            let foreign_type_pk_field_name = &foreign_pk_field_id
                                .resolve(&subsystem.core_subsystem.entity_types)
                                .name;

                            if let Some(foreign_type_pk_arg) =
                                get_argument_field(argument_value, foreign_type_pk_field_name)
                            {
                                let value_column =
                                    cast::literal_column(foreign_type_pk_arg, self_column)
                                        .map_err(|e| {
                                            SubsystemRpcError::UserDisplayError(
                                                e.user_error_message(),
                                            )
                                        })?;
                                column_values.push((self_column_id, value_column));
                            }
                        }
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }

    Ok(column_values)
}

macro_rules! impl_update_resolver {
    ($($ty:ty),+) => { $(
        #[async_trait]
        impl OperationResolver for $ty {
            async fn resolve<'a>(
                &'a self,
                validated_params: &mut HashMap<String, Val>,
                request_context: &'a RequestContext<'a>,
                subsystem: &'a PostgresRpcSubsystemWithRouter,
            ) -> Result<PgAbstractOperation, SubsystemRpcError> {
                resolve_update_predicate_params(
                    &self.parameters.predicate_params,
                    &self.parameters.data_param.name,
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

impl_update_resolver!(PkUpdate, UniqueUpdate);

#[async_trait]
impl OperationResolver for CollectionUpdate {
    async fn resolve<'a>(
        &'a self,
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<PgAbstractOperation, SubsystemRpcError> {
        let entity_type = self.return_type.typ(&subsystem.core_subsystem.entity_types);
        let data_param_name = &self.parameters.data_param.name;

        let data_val = validated_params
            .remove(data_param_name.as_str())
            .ok_or_else(|| {
                SubsystemRpcError::InvalidParams(format!(
                    "Missing required parameter: {data_param_name}"
                ))
            })?;

        let query_predicate = resolve_optional_predicate_param(
            &self.parameters.predicate_param,
            validated_params,
            request_context,
            subsystem,
        )
        .await?;

        build_update_operation(
            entity_type,
            &self.return_type,
            data_val,
            query_predicate,
            request_context,
            subsystem,
        )
        .await
    }
}

#[async_trait]
impl OperationSelectionResolver for CollectionQuery {
    async fn resolve_select<'a>(
        &'a self,
        validated_params: &mut HashMap<String, Val>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresRpcSubsystemWithRouter,
    ) -> Result<PgAbstractSelect, SubsystemRpcError> {
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
) -> Result<PgAbstractPredicate, SubsystemRpcError> {
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
) -> Result<PgAbstractPredicate, SubsystemRpcError> {
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
) -> Result<PgAbstractSelect, SubsystemRpcError> {
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
            ) -> Result<PgAbstractSelect, SubsystemRpcError> {
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

/// Compute all nested operations (create/update/delete) for OneToMany relations in an update.
async fn compute_nested_update_ops<'a>(
    entity_type: &EntityType,
    data_val: &'a Val,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<
    (
        Vec<PgNestedAbstractUpdate>,
        Vec<PgNestedAbstractInsertSet>,
        Vec<PgNestedAbstractDelete>,
    ),
    SubsystemRpcError,
> {
    let mut nested_updates = Vec::new();
    let mut nested_inserts = Vec::new();
    let mut nested_deletes = Vec::new();

    for field in &entity_type.fields {
        let PostgresRelation::OneToMany(OneToManyRelation {
            relation_id,
            foreign_entity_id,
            ..
        }) = &field.relation
        else {
            continue;
        };

        let Some(ops_val) = get_argument_field(data_val, &field.name) else {
            continue;
        };

        let foreign_entity = &subsystem.core_subsystem.entity_types[*foreign_entity_id];
        let nesting_relation = relation_id.deref(&subsystem.core_subsystem.database);

        // Handle "create" sub-field
        if let Some(create_arg) = get_argument_field(ops_val, "create") {
            nested_inserts.push(
                compute_nested_create_for_update(
                    foreign_entity,
                    entity_type,
                    create_arg,
                    &nesting_relation,
                    request_context,
                    subsystem,
                )
                .await?,
            );
        }

        // Handle "update" sub-field
        if let Some(update_arg) = get_argument_field(ops_val, "update") {
            nested_updates.extend(
                compute_nested_update_items(
                    foreign_entity,
                    update_arg,
                    &nesting_relation,
                    request_context,
                    subsystem,
                )
                .await?,
            );
        }

        // Handle "delete" sub-field
        if let Some(delete_arg) = get_argument_field(ops_val, "delete") {
            nested_deletes.extend(
                compute_nested_delete_items(
                    foreign_entity,
                    delete_arg,
                    &nesting_relation,
                    request_context,
                    subsystem,
                )
                .await?,
            );
        }
    }

    Ok((nested_updates, nested_inserts, nested_deletes))
}

/// Compute nested create operations within an update (the "create" sub-field).
async fn compute_nested_create_for_update<'a>(
    foreign_entity: &EntityType,
    parent_entity: &EntityType,
    create_arg: &'a Val,
    nesting_relation: &OneToMany,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<PgNestedAbstractInsertSet, SubsystemRpcError> {
    let items = val_as_items(create_arg)?;

    let relation_column_ids: Vec<_> = nesting_relation
        .column_pairs
        .iter()
        .map(|pair| pair.foreign_column_id)
        .collect();

    let mut inserts = Vec::new();

    for item in items {
        let precheck =
            compute_create_access(foreign_entity, item, request_context, subsystem).await?;
        let elements = compute_create_columns(
            foreign_entity,
            item,
            Some(parent_entity),
            request_context,
            subsystem,
        )
        .await?;

        inserts.push(NestedAbstractInsert {
            relation_column_ids: relation_column_ids.clone(),
            insert: AbstractInsert {
                table_id: foreign_entity.table_id,
                rows: vec![InsertionRow { elems: elements }],
                precheck_predicates: vec![precheck],
                selection: AbstractSelect {
                    table_id: foreign_entity.table_id,
                    selection: Selection::Seq(vec![]),
                    predicate: AbstractPredicate::True,
                    order_by: None,
                    offset: None,
                    limit: None,
                },
            },
        });
    }

    // Compute a filter_predicate from the child entity's access control to restrict
    // which parent rows get the nested insert. This mirrors what GraphQL does at build time
    // via parent_predicate() — we do it at runtime since RPC has no build step for mutations.
    let child_update_access_expr = &subsystem.core_subsystem.database_access_expressions
        [foreign_entity.access.update.database];
    let parent_access_expr = postgres_core_model::access::parent_predicate(
        child_update_access_expr.clone(),
        parent_entity,
    )
    .map_err(SubsystemRpcError::UserDisplayError)?;
    let filter_predicate = solve_access(&parent_access_expr, request_context, subsystem).await?;

    Ok(NestedAbstractInsertSet::new(inserts, filter_predicate))
}

/// Compute nested update items (the "update" sub-field).
async fn compute_nested_update_items<'a>(
    foreign_entity: &EntityType,
    update_arg: &'a Val,
    nesting_relation: &OneToMany,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<Vec<PgNestedAbstractUpdate>, SubsystemRpcError> {
    let items = val_as_items(update_arg)?;

    let mut updates = Vec::new();

    for item in items {
        let (precheck_predicate, entity_predicate) =
            compute_update_access(foreign_entity, item, request_context, subsystem).await?;

        // Build predicate from PK values for row identification
        let arg_predicate = build_pk_predicate(foreign_entity, item, subsystem)?;
        let predicate = AbstractPredicate::and(arg_predicate, entity_predicate);

        // Compute non-PK column values (reuses compute_update_columns which skips PKs)
        let update_columns = compute_update_columns(foreign_entity, item, subsystem)?;

        updates.push(NestedAbstractUpdate {
            nesting_relation: nesting_relation.clone(),
            update: AbstractUpdate {
                table_id: foreign_entity.table_id,
                predicate,
                column_values: update_columns,
                selection: AbstractSelect {
                    table_id: foreign_entity.table_id,
                    selection: Selection::Seq(vec![]),
                    predicate: AbstractPredicate::True,
                    order_by: None,
                    offset: None,
                    limit: None,
                },
                nested_updates: vec![],
                nested_inserts: vec![],
                nested_deletes: vec![],
                precheck_predicates: vec![precheck_predicate],
            },
        });
    }

    Ok(updates)
}

/// Compute nested delete items (the "delete" sub-field).
async fn compute_nested_delete_items<'a>(
    foreign_entity: &EntityType,
    delete_arg: &'a Val,
    nesting_relation: &OneToMany,
    request_context: &'a RequestContext<'a>,
    subsystem: &'a PostgresRpcSubsystemWithRouter,
) -> Result<Vec<PgNestedAbstractDelete>, SubsystemRpcError> {
    let items = val_as_items(delete_arg)?;

    // Check delete access once (does not depend on individual items)
    let access_predicate = compute_entity_access_predicate(
        foreign_entity,
        AccessKind::Delete,
        request_context,
        subsystem,
    )
    .await?;

    let mut deletes = Vec::new();

    for item in items {
        // Extract PK values and build predicate
        let pk_predicate = build_pk_predicate(foreign_entity, item, subsystem)?;
        let predicate = AbstractPredicate::and(pk_predicate, access_predicate.clone());

        deletes.push(NestedAbstractDelete {
            nesting_relation: nesting_relation.clone(),
            delete: AbstractDelete {
                table_id: foreign_entity.table_id,
                predicate,
                selection: AbstractSelect {
                    table_id: foreign_entity.table_id,
                    selection: Selection::Seq(vec![]),
                    predicate: AbstractPredicate::True,
                    order_by: None,
                    offset: None,
                    limit: None,
                },
                precheck_predicates: vec![AbstractPredicate::True],
            },
        });
    }

    Ok(deletes)
}

/// Extract items from a Val::List.
fn val_as_items(val: &Val) -> Result<Vec<&Val>, SubsystemRpcError> {
    match val {
        Val::List(items) => Ok(items.iter().collect()),
        other => Err(SubsystemRpcError::UserDisplayError(format!(
            "Expected an array, got {:?}",
            other
        ))),
    }
}

/// Build a predicate from PK values in the given data value.
fn build_pk_predicate(
    entity_type: &EntityType,
    data_val: &Val,
    subsystem: &PostgresRpcSubsystemWithRouter,
) -> Result<PgAbstractPredicate, SubsystemRpcError> {
    let mut predicate = AbstractPredicate::True;

    for pk_field in entity_type.pk_fields() {
        let PostgresRelation::Scalar { column_id, .. } = &pk_field.relation else {
            continue;
        };
        let Some(value) = get_argument_field(data_val, &pk_field.name) else {
            return Err(SubsystemRpcError::UserDisplayError(format!(
                "Missing required primary key field '{}' for nested operation.",
                pk_field.name
            )));
        };
        let column = column_id.get_column(&subsystem.core_subsystem.database);
        let value_column = cast::literal_column(value, column)
            .map_err(|e| SubsystemRpcError::UserDisplayError(e.user_error_message()))?;
        let Column::Param(param) = value_column else {
            return Err(SubsystemRpcError::UserDisplayError(format!(
                "Expected a literal value for PK field '{}'",
                pk_field.name
            )));
        };
        let value_path = ColumnPath::Param(param);
        predicate = AbstractPredicate::and(
            predicate,
            AbstractPredicate::eq(
                ColumnPath::Physical(PhysicalColumnPath::leaf(*column_id)),
                value_path,
            ),
        );
    }

    Ok(predicate)
}

fn from_postgres_error(e: PostgresExecutionError) -> SubsystemRpcError {
    match e {
        PostgresExecutionError::Authorization => SubsystemRpcError::Authorization,
        PostgresExecutionError::Postgres(exo_sql::database_error::DatabaseError::Precheck(_)) => {
            SubsystemRpcError::Authorization
        }
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
