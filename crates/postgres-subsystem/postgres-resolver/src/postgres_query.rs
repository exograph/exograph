use super::{
    postgres_execution_error::PostgresExecutionError,
    sql_mapper::SQLOperationKind,
    util::{check_access, Arguments},
};
use crate::{
    operation_resolver::OperationResolver, order_by_mapper::OrderByParameterInput,
    sql_mapper::extract_and_map,
};
use async_recursion::async_recursion;
use async_trait::async_trait;
use core_plugin_interface::core_resolver::{
    request_context::RequestContext, validation::field::ValidatedField,
};
use futures::StreamExt;
use payas_sql::{
    AbstractOperation, AbstractOrderBy, AbstractPredicate, AbstractSelect, ColumnPathLink,
    ColumnSelection, SelectionCardinality, SelectionElement,
};
use postgres_model::{
    limit_offset::{LimitParameter, OffsetParameter},
    model::ModelPostgresSystem,
    operation::{CollectionQuery, OperationReturnType, PkQuery},
    order::OrderByParameter,
    predicate::PredicateParameter,
    relation::{PostgresRelation, RelationCardinality},
    types::{PostgresType, PostgresTypeKind, PostgresTypeModifier},
};

#[async_trait]
impl OperationResolver for PkQuery {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<AbstractOperation<'a>, PostgresExecutionError> {
        let abstract_select = compute_select(
            &self.parameter.predicate_param,
            &None,
            &None,
            &None,
            &self.return_type,
            field,
            subsystem,
            request_context,
        )
        .await?;

        Ok(AbstractOperation::Select(abstract_select))
    }
}

#[async_trait]
impl OperationResolver for CollectionQuery {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<AbstractOperation<'a>, PostgresExecutionError> {
        let parameter = &self.parameter;
        let abstract_select = compute_select(
            &parameter.predicate_param,
            &parameter.order_by_param,
            &parameter.limit_param,
            &parameter.offset_param,
            &self.return_type,
            field,
            subsystem,
            request_context,
        )
        .await?;

        Ok(AbstractOperation::Select(abstract_select))
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn compute_select<'content>(
    predicate_param: &'content Option<PredicateParameter>,
    order_by_param: &'content Option<OrderByParameter>,
    limit_param: &Option<LimitParameter>,
    offset_param: &Option<OffsetParameter>,
    return_type: &OperationReturnType,
    field: &'content ValidatedField,
    subsystem: &'content ModelPostgresSystem,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractSelect<'content>, PostgresExecutionError> {
    let access_predicate = check_access(
        return_type,
        &SQLOperationKind::Retrieve,
        subsystem,
        request_context,
    )
    .await?;

    let query_predicate = super::predicate_mapper::compute_predicate(
        predicate_param.as_ref(),
        &field.arguments,
        subsystem,
    )?;
    let predicate = AbstractPredicate::and(query_predicate, access_predicate);

    let order_by = compute_order_by(order_by_param, &field.arguments, subsystem)?;

    let limit = extract_and_map(limit_param.as_ref(), &field.arguments, subsystem)?;
    let offset = extract_and_map(offset_param.as_ref(), &field.arguments, subsystem)?;

    let return_postgres_type = return_type.typ(subsystem);

    let content_object = content_select(
        return_postgres_type,
        &field.subfields,
        subsystem,
        request_context,
    )
    .await?;

    let root_physical_table =
        if let PostgresTypeKind::Composite(composite_root_type) = &return_postgres_type.kind {
            &subsystem.tables[composite_root_type.table_id]
        } else {
            return Err(PostgresExecutionError::Generic(
                "Expected a composite type".into(),
            ));
        };

    let selection_cardinality = match return_type.type_modifier {
        PostgresTypeModifier::Optional | PostgresTypeModifier::NonNull => SelectionCardinality::One,
        PostgresTypeModifier::List => SelectionCardinality::Many,
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
    order_by_param: &'content Option<OrderByParameter>,
    arguments: &'content Arguments,
    subsystem: &'content ModelPostgresSystem,
) -> Result<Option<AbstractOrderBy<'content>>, PostgresExecutionError> {
    extract_and_map(
        order_by_param.as_ref().map(|param| OrderByParameterInput {
            param,
            parent_column_path: None,
        }),
        arguments,
        subsystem,
    )
}

#[async_recursion]
async fn content_select<'content>(
    return_type: &PostgresType,
    fields: &'content [ValidatedField],
    subsystem: &'content ModelPostgresSystem,
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
    return_type: &PostgresType,
    field: &'content ValidatedField,
    subsystem: &'content ModelPostgresSystem,
    request_context: &'content RequestContext<'content>,
) -> Result<ColumnSelection<'content>, PostgresExecutionError> {
    let selection_elem = if field.name == "__typename" {
        SelectionElement::Constant(return_type.name.clone())
    } else {
        let model_field = return_type.model_field(&field.name).unwrap();

        match &model_field.relation {
            PostgresRelation::Pk { column_id } | PostgresRelation::Scalar { column_id } => {
                let column = column_id.get_column(subsystem);
                SelectionElement::Physical(column)
            }
            PostgresRelation::ManyToOne {
                column_id,
                other_type_id,
                ..
            } => {
                let other_type = &subsystem.postgres_types[*other_type_id];
                let other_table = &subsystem.tables[other_type.table_id().unwrap()];

                let other_table_pk_query = match &other_type.kind {
                    PostgresTypeKind::Primitive => panic!(""),
                    PostgresTypeKind::Composite(kind) => &subsystem.pk_queries[kind.pk_query],
                };
                let self_table = &subsystem.tables[return_type
                    .table_id()
                    .expect("No table for a composite type")];
                let relation_link = ColumnPathLink {
                    self_column: (column_id.get_column(subsystem), self_table),
                    linked_column: Some((
                        other_table
                            .get_pk_physical_column()
                            .expect("No primary key column found"),
                        other_table,
                    )),
                };

                let nested_abstract_select = compute_select(
                    &other_table_pk_query.parameter.predicate_param,
                    &None,
                    &None,
                    &None,
                    &other_table_pk_query.return_type,
                    field,
                    subsystem,
                    request_context,
                )
                .await?;
                SelectionElement::Nested(relation_link, nested_abstract_select)
            }
            PostgresRelation::OneToMany {
                other_type_column_id,
                other_type_id,
                cardinality,
            } => {
                let other_type = &subsystem.postgres_types[*other_type_id];
                let (
                    other_table_predicate_param,
                    other_table_order_by_param,
                    other_table_limit_param,
                    other_table_offset_param,
                    other_table_return_type,
                ) = {
                    match &other_type.kind {
                        PostgresTypeKind::Primitive => panic!(""),
                        PostgresTypeKind::Composite(kind) => {
                            // Get an appropriate query based on the cardinality of the relation
                            if cardinality == &RelationCardinality::Unbounded {
                                let collection_query =
                                    &subsystem.collection_queries[kind.collection_query];
                                let parameter = &collection_query.parameter;

                                (
                                    &parameter.predicate_param,
                                    &parameter.order_by_param,
                                    &parameter.limit_param,
                                    &parameter.offset_param,
                                    &collection_query.return_type,
                                )
                            } else {
                                let pk_query = &subsystem.pk_queries[kind.pk_query];
                                let parameter = &pk_query.parameter;

                                (
                                    &parameter.predicate_param,
                                    &None,
                                    &None,
                                    &None,
                                    &pk_query.return_type,
                                )
                            }
                        }
                    }
                };
                let self_table = &subsystem.tables[return_type.table_id().unwrap()];
                let self_table_pk_column = self_table
                    .get_pk_physical_column()
                    .expect("No primary key column found");
                let relation_link = ColumnPathLink {
                    self_column: (self_table_pk_column, self_table),
                    linked_column: Some((
                        other_type_column_id.get_column(subsystem),
                        &subsystem.tables[other_type.table_id().unwrap()],
                    )),
                };
                let nested_abstract_select = compute_select(
                    other_table_predicate_param,
                    other_table_order_by_param,
                    other_table_limit_param,
                    other_table_offset_param,
                    other_table_return_type,
                    field,
                    subsystem,
                    request_context,
                )
                .await?;
                SelectionElement::Nested(relation_link, nested_abstract_select)
            }
        }
    };

    Ok(ColumnSelection::new(field.output_name(), selection_elem))
}
