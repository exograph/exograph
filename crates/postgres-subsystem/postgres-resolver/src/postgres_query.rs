use super::{
    postgres_execution_error::PostgresExecutionError,
    sql_mapper::SQLOperationKind,
    util::{check_access, Arguments},
};
use crate::{
    operation_resolver::OperationSelectionResolver, order_by_mapper::OrderByParameterInput,
    sql_mapper::extract_and_map,
};
use async_recursion::async_recursion;
use async_trait::async_trait;
use core_plugin_interface::core_model::types::OperationReturnType;
use core_plugin_interface::core_resolver::{
    request_context::RequestContext, validation::field::ValidatedField,
};
use futures::StreamExt;
use payas_sql::{
    AbstractOrderBy, AbstractPredicate, AbstractSelect, ColumnPathLink, ColumnSelection, Limit,
    Offset, SelectionCardinality, SelectionElement,
};
use postgres_model::{
    aggregate::AggregateField,
    operation::{CollectionQuery, CollectionQueryParameter, PkQuery},
    order::OrderByParameter,
    predicate::PredicateParameter,
    relation::{PostgresRelation, RelationCardinality},
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
    ) -> Result<AbstractSelect<'a>, PostgresExecutionError> {
        compute_select(
            &self.parameters.predicate_param,
            None,
            None,
            None,
            &self.return_type,
            field,
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
    ) -> Result<AbstractSelect<'a>, PostgresExecutionError> {
        let CollectionQueryParameter {
            predicate_param,
            order_by_param,
            limit_param,
            offset_param,
        } = &self.parameters;

        compute_select(
            predicate_param,
            compute_order_by(order_by_param, &field.arguments, subsystem)?,
            extract_and_map(limit_param, &field.arguments, subsystem)?,
            extract_and_map(offset_param, &field.arguments, subsystem)?,
            &self.return_type,
            field,
            subsystem,
            request_context,
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
async fn compute_select<'content>(
    predicate_param: &'content PredicateParameter,
    order_by: Option<AbstractOrderBy<'content>>,
    limit: Option<Limit>,
    offset: Option<Offset>,
    return_type: &'content OperationReturnType<EntityType>,
    field: &'content ValidatedField,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractSelect<'content>, PostgresExecutionError> {
    let access_predicate = check_access(
        return_type,
        &SQLOperationKind::Retrieve,
        subsystem,
        request_context,
    )
    .await?;

    let query_predicate =
        super::predicate_mapper::compute_predicate(predicate_param, &field.arguments, subsystem)?;
    let predicate = AbstractPredicate::and(query_predicate, access_predicate);

    let return_postgres_type = return_type.typ(&subsystem.entity_types);

    let content_object = content_select(
        return_postgres_type,
        &field.subfields,
        subsystem,
        request_context,
    )
    .await?;

    let root_physical_table = &subsystem.tables[return_postgres_type.table_id];

    let selection_cardinality = match return_type {
        OperationReturnType::List(_) => SelectionCardinality::Many,
        _ => SelectionCardinality::One,
    };
    Ok(AbstractSelect {
        table: root_physical_table,
        selection: payas_sql::Selection::Json(content_object, selection_cardinality),
        predicate,
        order_by,
        offset,
        limit,
    })
}

fn compute_order_by<'content>(
    param: &'content OrderByParameter,
    arguments: &'content Arguments,
    subsystem: &'content PostgresSubsystem,
) -> Result<Option<AbstractOrderBy<'content>>, PostgresExecutionError> {
    extract_and_map(
        OrderByParameterInput {
            param,
            parent_column_path: None,
        },
        arguments,
        subsystem,
    )
}

#[async_recursion]
async fn content_select<'content>(
    return_type: &EntityType,
    fields: &'content [ValidatedField],
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<Vec<ColumnSelection<'content>>, PostgresExecutionError> {
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
) -> Result<ColumnSelection<'content>, PostgresExecutionError> {
    let selection_elem = if field.name == "__typename" {
        SelectionElement::Constant(return_type.name.to_owned())
    } else {
        let entity_field = return_type.field(&field.name);

        match entity_field {
            Some(entity_field) => {
                map_persistent_field(entity_field, return_type, field, subsystem, request_context)
                    .await?
            }
            None => {
                let agg_field = return_type.aggregate_field(&field.name).unwrap();
                map_aggregate_field(agg_field, return_type, field, subsystem, request_context)
                    .await?
            }
        }
    };

    Ok(ColumnSelection::new(field.output_name(), selection_elem))
}

async fn map_persistent_field<'content>(
    entity_field: &PostgresField<EntityType>,
    return_type: &EntityType,
    field: &'content ValidatedField,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<SelectionElement<'content>, PostgresExecutionError> {
    match &entity_field.relation {
        PostgresRelation::Pk { column_id } | PostgresRelation::Scalar { column_id } => {
            let column = column_id.get_column(subsystem);
            Ok(SelectionElement::Physical(column))
        }
        PostgresRelation::ManyToOne {
            column_id,
            other_type_id,
            ..
        } => {
            let other_type = &subsystem.entity_types[*other_type_id];
            let other_table = &subsystem.tables[other_type.table_id];

            let other_table_pk_query = &subsystem.pk_queries[other_type.pk_query];
            let self_table = &subsystem.tables[return_type.table_id];
            let relation_link = ColumnPathLink {
                self_column: (column_id.get_column(subsystem), self_table),
                linked_column: Some((
                    other_table
                        .get_pk_physical_column()
                        .expect("No primary key column found"),
                    other_table,
                )),
            };

            let nested_abstract_select = other_table_pk_query
                .resolve_select(field, request_context, subsystem)
                .await?;

            Ok(SelectionElement::Nested(
                relation_link,
                nested_abstract_select,
            ))
        }
        PostgresRelation::OneToMany {
            other_type_column_id,
            other_type_id,
            cardinality,
        } => {
            let other_type = &subsystem.entity_types[*other_type_id];
            let self_table = &subsystem.tables[return_type.table_id];
            let self_table_pk_column = self_table
                .get_pk_physical_column()
                .expect("No primary key column found");
            let relation_link = ColumnPathLink {
                self_column: (self_table_pk_column, self_table),
                linked_column: Some((
                    other_type_column_id.get_column(subsystem),
                    &subsystem.tables[other_type.table_id],
                )),
            };

            let nested_abstract_select = {
                // Get an appropriate query based on the cardinality of the relation
                if cardinality == &RelationCardinality::Unbounded {
                    let collection_query =
                        &subsystem.collection_queries[other_type.collection_query];

                    collection_query
                        .resolve_select(field, request_context, subsystem)
                        .await?
                } else {
                    let pk_query = &subsystem.pk_queries[other_type.pk_query];

                    pk_query
                        .resolve_select(field, request_context, subsystem)
                        .await?
                }
            };

            Ok(SelectionElement::Nested(
                relation_link,
                nested_abstract_select,
            ))
        }
    }
}

async fn map_aggregate_field<'content>(
    agg_field: &AggregateField,
    return_type: &EntityType,
    field: &'content ValidatedField,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<SelectionElement<'content>, PostgresExecutionError> {
    if let Some(PostgresRelation::OneToMany {
        other_type_column_id,
        other_type_id,
        cardinality,
    }) = &agg_field.relation
    {
        // TODO: Avoid code duplication with map_persistent_field
        let other_type = &subsystem.entity_types[*other_type_id];
        let self_table = &subsystem.tables[return_type.table_id];
        let self_table_pk_column = self_table
            .get_pk_physical_column()
            .expect("No primary key column found");
        let relation_link = ColumnPathLink {
            self_column: (self_table_pk_column, self_table),
            linked_column: Some((
                other_type_column_id.get_column(subsystem),
                &subsystem.tables[other_type.table_id],
            )),
        };

        let nested_abstract_select = {
            // Aggregate is supported only for unbounded relations (i.e. not supported for one-to-one)
            if cardinality == &RelationCardinality::Unbounded {
                let aggregate_query = &subsystem.aggregate_queries[other_type.aggregate_query];

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

        Ok(SelectionElement::Nested(
            relation_link,
            nested_abstract_select,
        ))
    } else {
        // Reaching this point means our validation logic failed
        Err(PostgresExecutionError::Generic(
            "Validation error: Aggregate is supported only for one-to-many".to_string(),
        ))
    }
}
