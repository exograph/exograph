use async_recursion::async_recursion;
use futures::StreamExt;

use core_resolver::request_context::RequestContext;
use core_resolver::system_resolver::SystemResolver;
use core_resolver::validation::field::ValidatedField;
use postgres_model::{
    model::ModelPostgresSystem,
    operation::{PostgresQuery, PostgresQueryParameter},
    relation::{PostgresRelation, RelationCardinality},
    types::{PostgresTypeKind, PostgresTypeModifier},
};

use payas_sql::{
    AbstractOrderBy, AbstractPredicate, AbstractSelect, ColumnPathLink, ColumnSelection, Limit,
    Offset, SelectionCardinality, SelectionElement,
};

use crate::util::find_arg;

use super::{
    order_by_mapper::OrderByParameterMapper,
    postgres_execution_error::PostgresExecutionError,
    sql_mapper::{SQLMapper, SQLOperationKind},
    util::{compute_sql_access_predicate, Arguments},
};

pub async fn compute_select<'content>(
    query: &'content PostgresQuery,
    field: &'content ValidatedField,
    additional_predicate: AbstractPredicate<'content>,
    subsystem: &'content ModelPostgresSystem,
    system_resolver: &'content SystemResolver,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractSelect<'content>, PostgresExecutionError> {
    let PostgresQueryParameter {
        predicate_param, ..
    } = &query.parameter;
    let access_predicate = compute_sql_access_predicate(
        &query.return_type,
        &SQLOperationKind::Retrieve,
        subsystem,
        request_context,
    )
    .await;

    if access_predicate == AbstractPredicate::False {
        return Err(PostgresExecutionError::Authorization);
    }

    let predicate = super::predicate_mapper::compute_predicate(
        predicate_param.as_ref(),
        &field.arguments,
        additional_predicate,
        subsystem,
        system_resolver,
    )
    .map_err(|e| match e {
        PostgresExecutionError::Validation(message) => PostgresExecutionError::Validation(format!(
            "Error computing predicate for field '{}': {}",
            field.name, message
        )),
        e => e,
    })?;

    let order_by = compute_order_by(query, &field.arguments, subsystem)?;

    let predicate = AbstractPredicate::and(predicate, access_predicate);

    let content_object = content_select(
        query,
        &field.subfields,
        subsystem,
        system_resolver,
        request_context,
    )
    .await?;

    let limit = compute_limit(query, &field.arguments, subsystem);
    let offset = compute_offset(query, &field.arguments, subsystem);

    let root_physical_table = if let PostgresTypeKind::Composite(composite_root_type) =
        &query.return_type.typ(subsystem).kind
    {
        &subsystem.tables[composite_root_type.table_id]
    } else {
        return Err(PostgresExecutionError::Generic(
            "Expected a composite type".into(),
        ));
    };

    let selection_cardinality = match query.return_type.type_modifier {
        PostgresTypeModifier::Optional | PostgresTypeModifier::NonNull => SelectionCardinality::One,
        PostgresTypeModifier::List => SelectionCardinality::Many,
    };
    let aselect = AbstractSelect {
        table: root_physical_table,
        selection: payas_sql::Selection::Json(content_object, selection_cardinality),
        predicate: Some(predicate),
        order_by,
        offset,
        limit,
    };

    Ok(aselect)
}

fn compute_order_by<'content>(
    query: &'content PostgresQuery,
    arguments: &'content Arguments,
    subsystem: &'content ModelPostgresSystem,
) -> Result<Option<AbstractOrderBy<'content>>, PostgresExecutionError> {
    let PostgresQueryParameter { order_by_param, .. } = &query.parameter;
    order_by_param
        .as_ref()
        .and_then(|order_by_param| {
            let argument_value = find_arg(arguments, &order_by_param.name);
            argument_value.map(|argument_value| {
                order_by_param.map_to_order_by(argument_value, None, subsystem)
            })
        })
        .transpose()
}

#[async_recursion]
async fn content_select<'content>(
    query: &PostgresQuery,
    fields: &'content [ValidatedField],
    subsystem: &'content ModelPostgresSystem,
    system_resolver: &'content SystemResolver,
    request_context: &'content RequestContext<'content>,
) -> Result<Vec<ColumnSelection<'content>>, PostgresExecutionError> {
    futures::stream::iter(fields.iter())
        .then(|field| async {
            map_field(query, field, subsystem, system_resolver, request_context).await
        })
        .collect::<Vec<Result<_, _>>>()
        .await
        .into_iter()
        .collect()
}

fn compute_limit<'content>(
    query: &'content PostgresQuery,
    arguments: &'content Arguments,
    subsystem: &'content ModelPostgresSystem,
) -> Option<Limit> {
    let PostgresQueryParameter { limit_param, .. } = &query.parameter;
    limit_param
        .as_ref()
        .and_then(|limit_param| {
            let argument_value = find_arg(arguments, &limit_param.name);
            argument_value.map(|argument_value| limit_param.map_to_sql(argument_value, subsystem))
        })
        .transpose()
        .unwrap()
}

fn compute_offset<'content>(
    query: &'content PostgresQuery,
    arguments: &'content Arguments,
    subsystem: &'content ModelPostgresSystem,
) -> Option<Offset> {
    let PostgresQueryParameter { offset_param, .. } = &query.parameter;
    offset_param
        .as_ref()
        .and_then(|offset_param| {
            let argument_value = find_arg(arguments, &offset_param.name);
            argument_value.map(|argument_value| offset_param.map_to_sql(argument_value, subsystem))
        })
        .transpose()
        .unwrap()
}

async fn map_field<'content>(
    query: &PostgresQuery,
    field: &'content ValidatedField,
    subsystem: &'content ModelPostgresSystem,
    system_resolver: &'content SystemResolver,
    request_context: &'content RequestContext<'content>,
) -> Result<ColumnSelection<'content>, PostgresExecutionError> {
    let return_type = query.return_type.typ(subsystem);

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
                    PostgresTypeKind::Composite(kind) => &subsystem.queries[kind.pk_query],
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
                    other_table_pk_query,
                    field,
                    AbstractPredicate::True,
                    subsystem,
                    system_resolver,
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
                let other_table_query = {
                    match &other_type.kind {
                        PostgresTypeKind::Primitive => panic!(""),
                        PostgresTypeKind::Composite(kind) => {
                            // Get an appropriate query based on the cardinality of the relation
                            if cardinality == &RelationCardinality::Unbounded {
                                &subsystem.queries[kind.collection_query]
                            } else {
                                &subsystem.queries[kind.pk_query]
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
                    other_table_query,
                    field,
                    AbstractPredicate::True,
                    subsystem,
                    system_resolver,
                    request_context,
                )
                .await?;
                SelectionElement::Nested(relation_link, nested_abstract_select)
            }
        }
    };

    Ok(ColumnSelection::new(field.output_name(), selection_elem))
}
