// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use async_recursion::async_recursion;
use async_trait::async_trait;
use common::context::RequestContext;
use common::value::Val;
use core_resolver::access_solver::{AccessInput, MissingValuePolicy};
use exo_sql_pg::{
    AbstractInsert, ColumnId, ColumnValuePair, InsertionElement, InsertionRow, ManyToOne,
    NestedInsertion, PgAbstractInsert, PgAbstractSelect, PgInsertionElement, PgInsertionRow,
};
use futures::future::{join_all, try_join_all};
use postgres_core_model::relation::{ManyToOneRelation, OneToManyRelation, PostgresRelation};
use postgres_core_model::types::{PostgresField, PostgresType, base_type};
use postgres_graphql_model::{
    mutation::DataParameter, subsystem::PostgresGraphQLSubsystem, types::MutationType,
};

use crate::{
    auth_util::check_access,
    sql_mapper::{SQLMapper, SQLOperationKind},
};

use postgres_core_resolver::{
    cast,
    create_util::{CreateFieldValue, create_field_value},
    postgres_execution_error::{PostgresExecutionError, WithContext},
};

pub struct InsertOperation<'a> {
    pub data_param: &'a DataParameter,
    pub select: PgAbstractSelect,
}

#[async_trait]
impl<'a> SQLMapper<'a, PgAbstractInsert> for InsertOperation<'a> {
    async fn to_sql(
        self,
        argument: &'a Val,
        subsystem: &'a PostgresGraphQLSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<PgAbstractInsert, PostgresExecutionError> {
        let data_type = &subsystem.mutation_types[self.data_param.typ.innermost().type_id];
        let table_id = subsystem.core_subsystem.entity_types[data_type.entity_id].table_id;

        let rows = map_argument(data_type, argument, None, subsystem, request_context).await?;

        Ok(AbstractInsert {
            table_id,
            rows,
            selection: self.select,
        })
    }

    fn param_name(&self) -> &str {
        &self.data_param.name
    }
}

/// `parent_reference` is the field referring to the parent when this is a nested create
pub(crate) async fn map_argument<'a>(
    data_type: &'a MutationType,
    argument: &'a Val,
    parent_reference: Option<&'a str>,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<Vec<PgInsertionRow>, PostgresExecutionError> {
    match argument {
        Val::List(arguments) => {
            let mapped = arguments.iter().map(|argument| {
                map_single(
                    data_type,
                    argument,
                    parent_reference,
                    subsystem,
                    request_context,
                )
            });
            try_join_all(mapped).await
        }
        _ => {
            let insertion_row = map_single(
                data_type,
                argument,
                parent_reference,
                subsystem,
                request_context,
            )
            .await?;
            Ok(vec![insertion_row])
        }
    }
}

/// Map a single item from the data parameter
#[async_recursion]
async fn map_single<'a>(
    data_type: &'a MutationType,
    argument: &'a Val,
    parent_reference: Option<&'a str>,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<PgInsertionRow, PostgresExecutionError> {
    let (precheck_predicate, _entity_predicate) = check_access(
        &subsystem.core_subsystem.entity_types[data_type.entity_id],
        &[],
        &SQLOperationKind::Create,
        subsystem,
        request_context,
        Some(&AccessInput {
            value: argument,
            missing_value_policy: MissingValuePolicy::Create { parent_reference },
            aliases: HashMap::new(),
        }),
    )
    .await?;

    use futures::stream::{self, StreamExt};

    let mapped = stream::iter(&data_type.fields)
        .map(|field| async move {
            let field_value =
                create_field_value(field, argument, &subsystem.core_subsystem, request_context)
                    .await
                    .ok()?;

            field_value.map(|field_value| async move {
                match &field.relation {
                    PostgresRelation::Scalar { column_id, .. } => {
                        vec![map_self_column(*column_id, field, field_value, subsystem).await]
                    }

                    PostgresRelation::ManyToOne {
                        relation: ManyToOneRelation { relation_id, .. },
                        ..
                    } => {
                        let ManyToOne { column_pairs, .. } =
                            relation_id.deref(&subsystem.core_subsystem.database);

                        let mapped = column_pairs.iter().map(|column_pair| {
                            map_self_column(
                                column_pair.self_column_id,
                                field,
                                field_value,
                                subsystem,
                            )
                        });
                        join_all(mapped).await
                    }

                    PostgresRelation::OneToMany(one_to_many_relation) => {
                        vec![
                            map_foreign(
                                field,
                                field_value.value(),
                                one_to_many_relation,
                                subsystem,
                                request_context,
                            )
                            .await,
                        ]
                    }

                    PostgresRelation::Embedded => {
                        panic!("Embedded relations cannot be used in create operations")
                    }
                }
            })
        })
        .collect::<Vec<_>>()
        .await;

    let row = join_all(mapped).await;
    let row = row.into_iter().flatten().collect::<Vec<_>>();
    let row: Result<Vec<PgInsertionElement>, PostgresExecutionError> =
        join_all(row).await.into_iter().flatten().collect();

    Ok(InsertionRow {
        elems: row?,
        precheck_predicate,
    })
}

async fn map_self_column<'a>(
    key_column_id: ColumnId,
    field: &'a PostgresField<MutationType>,
    field_value: CreateFieldValue<'a>,
    subsystem: &'a PostgresGraphQLSubsystem,
) -> Result<PgInsertionElement, PostgresExecutionError> {
    let key_column = key_column_id.get_column(&subsystem.core_subsystem.database);

    let argument_value = match &field.relation {
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

            if column_pairs.len() != foreign_pk_field_ids.len() {
                return Err(PostgresExecutionError::Generic(format!(
                    "Mismatch between number of columns in relation ({}) and number of foreign PK fields ({})",
                    column_pairs.len(),
                    foreign_pk_field_ids.len()
                )));
            }

            let foreign_type_pk_field_name = column_pairs
                .iter()
                .zip(foreign_pk_field_ids)
                .find_map(|(column_pair, foreign_pk_field_id)| {
                    if column_pair.self_column_id == key_column_id {
                        Some(
                            &foreign_pk_field_id
                                .resolve(&subsystem.core_subsystem.entity_types)
                                .name,
                        )
                    } else {
                        None
                    }
                })
                .expect("Foreign key field not found");

            // A null reference has no key, so it is stored as is
            field_value
                .reference_key(foreign_type_pk_field_name)
                .unwrap_or(field_value.value())
        }
        _ => field_value.value(),
    };

    let value_column = cast::literal_column(argument_value, key_column).with_context(format!(
        "trying to convert the '{}' field to the '{}' type",
        field.name,
        key_column.typ.type_string()
    ))?;

    Ok(InsertionElement::SelfInsert(ColumnValuePair::new(
        key_column_id,
        value_column,
    )))
}

/// Map foreign elements of a data parameter
/// For example, if the data parameter is `data: {name: "venue-name", concerts: [{<concert-info1>}, {<concert-info2>}]} }
/// this needs to be called for the `concerts` part (which is mapped to a separate table)
async fn map_foreign<'a>(
    field: &'a PostgresField<MutationType>, // "concerts"
    argument: &'a Val,                      // [{<concert-info1>}, {<concert-info2>}]
    one_to_many_relation: &OneToManyRelation,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<PgInsertionElement, PostgresExecutionError> {
    let field_type = base_type(
        &field.typ,
        &subsystem.core_subsystem.primitive_types,
        &subsystem.mutation_types,
    );

    let field_type = match field_type {
        PostgresType::Composite(field_type) => field_type,
        _ => unreachable!("Foreign type cannot be a primitive"), // TODO: Handle this at the type-level
    };

    let parent_reference = one_to_many_relation
        .foreign_field(&subsystem.core_subsystem.entity_types)
        .map(|foreign_field| foreign_field.name.as_str());

    let insertions = map_argument(
        field_type,
        argument,
        parent_reference,
        subsystem,
        request_context,
    )
    .await?;

    Ok(InsertionElement::NestedInsert(NestedInsertion {
        relation_id: one_to_many_relation.relation_id,
        insertions,
    }))
}
