// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::predicate_mapper::compute_predicate;
use super::{
    auth_util::check_access, postgres_execution_error::PostgresExecutionError,
    sql_mapper::SQLOperationKind, util::Arguments,
};
use crate::util::to_pg_vector;
use crate::{
    operation_resolver::OperationSelectionResolver, order_by_mapper::OrderByParameterInput,
    sql_mapper::extract_and_map,
};
use async_recursion::async_recursion;
use async_trait::async_trait;
use core_plugin_interface::core_model::types::OperationReturnType;
use core_plugin_interface::core_resolver::{
    context::RequestContext, validation::field::ValidatedField,
};
use exo_sql::{
    AbstractOrderBy, AbstractPredicate, AbstractSelect, AliasedSelectionElement, Function, Limit,
    Offset, RelationId, SQLParamContainer, SelectionCardinality, SelectionElement,
};
use futures::stream::TryStreamExt;
use futures::StreamExt;
use postgres_model::query::UniqueQuery;
use postgres_model::vector_distance::VectorDistanceField;
use postgres_model::{
    aggregate::AggregateField,
    order::OrderByParameter,
    query::{CollectionQuery, CollectionQueryParameters, PkQuery},
    relation::{ManyToOneRelation, OneToManyRelation, PostgresRelation, RelationCardinality},
    subsystem::PostgresSubsystem,
    types::{EntityType, PostgresField},
};

#[async_trait]
impl OperationSelectionResolver for PkQuery {
    async fn resolve_select<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresSubsystem,
    ) -> Result<AbstractSelect, PostgresExecutionError> {
        let predicate = compute_predicate(
            &self.parameters.predicate_param,
            &field.arguments,
            subsystem,
            request_context,
        )
        .await?;

        compute_select(
            predicate,
            None,
            None,
            None,
            &self.return_type,
            &field.subfields,
            subsystem,
            request_context,
        )
        .await
    }
}

#[async_trait]
impl OperationSelectionResolver for UniqueQuery {
    async fn resolve_select<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresSubsystem,
    ) -> Result<AbstractSelect, PostgresExecutionError> {
        let predicate = futures::stream::iter(
            self.parameters
                .predicate_params
                .iter()
                .map(Ok::<_, PostgresExecutionError>),
        )
        .try_fold(AbstractPredicate::True, |acc, p| async {
            let predicate =
                compute_predicate(p, &field.arguments, subsystem, request_context).await?;
            Ok(AbstractPredicate::and(acc, predicate))
        })
        .await?;

        compute_select(
            predicate,
            None,
            None,
            None,
            &self.return_type,
            &field.subfields,
            subsystem,
            request_context,
        )
        .await
    }
}

#[async_trait]
impl OperationSelectionResolver for CollectionQuery {
    async fn resolve_select<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresSubsystem,
    ) -> Result<AbstractSelect, PostgresExecutionError> {
        let CollectionQueryParameters {
            predicate_param,
            order_by_param,
            limit_param,
            offset_param,
        } = &self.parameters;

        let arguments = &field.arguments;

        compute_select(
            compute_predicate(predicate_param, arguments, subsystem, request_context).await?,
            compute_order_by(order_by_param, arguments, subsystem, request_context).await?,
            extract_and_map(limit_param, arguments, subsystem, request_context).await?,
            extract_and_map(offset_param, arguments, subsystem, request_context).await?,
            &self.return_type,
            &field.subfields,
            subsystem,
            request_context,
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn compute_select<'content>(
    predicate: AbstractPredicate,
    order_by: Option<AbstractOrderBy>,
    limit: Option<Limit>,
    offset: Option<Offset>,
    return_type: &'content OperationReturnType<EntityType>,
    selection: &'content [ValidatedField],
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractSelect, PostgresExecutionError> {
    let return_entity_type = return_type.typ(&subsystem.entity_types);

    let access_predicate = check_access(
        return_entity_type,
        selection,
        &SQLOperationKind::Retrieve,
        subsystem,
        request_context,
        None,
    )
    .await?;

    let predicate = AbstractPredicate::and(predicate, access_predicate);

    let content_object =
        content_select(return_entity_type, selection, subsystem, request_context).await?;

    let selection_cardinality = match return_type {
        OperationReturnType::List(_) => SelectionCardinality::Many,
        _ => SelectionCardinality::One,
    };
    Ok(AbstractSelect {
        table_id: return_entity_type.table_id,
        selection: exo_sql::Selection::Json(content_object, selection_cardinality),
        predicate,
        order_by,
        offset,
        limit,
    })
}

async fn compute_order_by<'content>(
    param: &'content OrderByParameter,
    arguments: &'content Arguments,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<Option<AbstractOrderBy>, PostgresExecutionError> {
    extract_and_map(
        OrderByParameterInput {
            param,
            parent_column_path: None,
        },
        arguments,
        subsystem,
        request_context,
    )
    .await
}

#[async_recursion]
async fn content_select<'content>(
    return_type: &EntityType,
    fields: &'content [ValidatedField],
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<Vec<AliasedSelectionElement>, PostgresExecutionError> {
    futures::stream::iter(fields.iter())
        .then(|field| async { map_field(return_type, field, subsystem, request_context).await })
        .collect::<Vec<Result<_, _>>>()
        .await
        .into_iter()
        .collect()
}

async fn map_field<'content>(
    return_type: &EntityType,
    field: &'content ValidatedField,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<AliasedSelectionElement, PostgresExecutionError> {
    let selection_elem = if field.name == "__typename" {
        SelectionElement::Constant(return_type.name.to_owned())
    } else {
        let entity_field = return_type.field_by_name(&field.name);

        match entity_field {
            Some(entity_field) => {
                map_persistent_field(entity_field, field, subsystem, request_context).await?
            }
            None => {
                let agg_field = return_type.aggregate_field_by_name(&field.name);
                match agg_field {
                    Some(agg_field) => {
                        map_aggregate_field(agg_field, field, subsystem, request_context).await?
                    }
                    None => {
                        let vector_distance_field = return_type
                            .vector_distance_field_by_name(&field.name)
                            .unwrap();

                        map_vector_distance_field(vector_distance_field, field).await?
                    }
                }
            }
        }
    };

    Ok(AliasedSelectionElement::new(
        field.output_name(),
        selection_elem,
    ))
}

async fn map_persistent_field<'content>(
    entity_field: &PostgresField<EntityType>,
    field: &'content ValidatedField,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<SelectionElement, PostgresExecutionError> {
    match &entity_field.relation {
        PostgresRelation::Pk { column_id } | PostgresRelation::Scalar { column_id } => {
            Ok(SelectionElement::Physical(*column_id))
        }
        PostgresRelation::ManyToOne(relation) => {
            let ManyToOneRelation {
                foreign_pk_field_id,
                ..
            } = relation;

            let foreign_type = &subsystem.entity_types[foreign_pk_field_id.entity_type_id()];

            let foreign_table_pk_query = &subsystem.pk_queries[foreign_type.pk_query];

            let nested_abstract_select = foreign_table_pk_query
                .resolve_select(field, request_context, subsystem)
                .await?;

            Ok(SelectionElement::SubSelect(
                RelationId::ManyToOne(relation.relation_id),
                nested_abstract_select,
            ))
        }
        PostgresRelation::OneToMany(relation) => {
            let OneToManyRelation {
                foreign_field_id,
                cardinality,
                ..
            } = relation;

            let foreign_type = &subsystem.entity_types[foreign_field_id.entity_type_id()];

            let nested_abstract_select = {
                // Get an appropriate query based on the cardinality of the relation
                if cardinality == &RelationCardinality::Unbounded {
                    let collection_query =
                        &subsystem.collection_queries[foreign_type.collection_query];

                    collection_query
                        .resolve_select(field, request_context, subsystem)
                        .await?
                } else {
                    let pk_query = &subsystem.pk_queries[foreign_type.pk_query];

                    pk_query
                        .resolve_select(field, request_context, subsystem)
                        .await?
                }
            };

            Ok(SelectionElement::SubSelect(
                RelationId::OneToMany(relation.relation_id),
                nested_abstract_select,
            ))
        }
    }
}

async fn map_aggregate_field<'content>(
    agg_field: &AggregateField,
    field: &'content ValidatedField,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<SelectionElement, PostgresExecutionError> {
    if let Some(PostgresRelation::OneToMany(relation)) = &agg_field.relation {
        let OneToManyRelation {
            foreign_field_id,
            cardinality,
            relation_id,
        } = relation;
        // TODO: Avoid code duplication with map_persistent_field
        let foreign_type = &subsystem.entity_types[foreign_field_id.entity_type_id()];

        let nested_abstract_select = {
            // Aggregate is supported only for unbounded relations (i.e. not supported for one-to-one)
            if cardinality == &RelationCardinality::Unbounded {
                let aggregate_query = &subsystem.aggregate_queries[foreign_type.aggregate_query];

                aggregate_query
                    .resolve_select(field, request_context, subsystem)
                    .await
            } else {
                // Reaching this point means our validation logic failed
                Err(PostgresExecutionError::Generic(
                    "Validation error: Aggregate is supported only for unbounded relations"
                        .to_string(),
                ))
            }
        }?;

        Ok(SelectionElement::SubSelect(
            RelationId::OneToMany(*relation_id),
            nested_abstract_select,
        ))
    } else {
        // Reaching this point means our validation logic failed
        Err(PostgresExecutionError::Generic(
            "Validation error: Aggregate is supported only for one-to-many".to_string(),
        ))
    }
}

async fn map_vector_distance_field<'content>(
    vector_distance_field: &VectorDistanceField,
    field: &'content ValidatedField,
) -> Result<SelectionElement, PostgresExecutionError> {
    let to_arg = field.arguments.get("to").ok_or_else(|| {
        PostgresExecutionError::Generic(
            "Missing 'to' argument for vector distance field".to_string(),
        )
    })?;

    let to_vector_value = to_pg_vector(to_arg, "to")?;

    Ok(SelectionElement::Function(Function::VectorDistance {
        column_id: vector_distance_field.column_id,
        distance_function: vector_distance_field.distance_function,
        target: SQLParamContainer::new(to_vector_value),
    }))
}
