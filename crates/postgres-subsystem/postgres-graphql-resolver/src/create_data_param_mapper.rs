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
use core_resolver::access_solver::AccessInput;
use core_resolver::context_extractor::ContextExtractor;
use exo_sql::{
    AbstractInsert, AbstractPredicate, AbstractSelect, ColumnId, ColumnValuePair, InsertionElement,
    InsertionRow, ManyToOne, NestedInsertion,
};
use futures::future::{join_all, try_join_all};
use postgres_core_model::relation::{ManyToOneRelation, OneToManyRelation, PostgresRelation};
use postgres_core_model::types::{
    PostgresField, PostgresFieldDefaultValue, PostgresType, base_type,
};
use postgres_graphql_model::{
    mutation::DataParameter, subsystem::PostgresGraphQLSubsystem, types::MutationType,
};

use crate::{
    auth_util::check_access,
    sql_mapper::{SQLMapper, SQLOperationKind},
};

use postgres_core_resolver::{
    cast,
    postgres_execution_error::{PostgresExecutionError, WithContext},
};

pub struct InsertOperation<'a> {
    pub data_param: &'a DataParameter,
    pub select: AbstractSelect,
}

#[async_trait]
impl<'a> SQLMapper<'a, AbstractInsert> for InsertOperation<'a> {
    async fn to_sql(
        self,
        argument: &'a Val,
        subsystem: &'a PostgresGraphQLSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractInsert, PostgresExecutionError> {
        let data_type = &subsystem.mutation_types[self.data_param.typ.innermost().type_id];
        let table_id = subsystem.core_subsystem.entity_types[data_type.entity_id].table_id;

        let (rows, precheck_predicates) =
            map_argument(data_type, argument, subsystem, request_context).await?;

        Ok(AbstractInsert {
            table_id,
            rows,
            selection: self.select,
            precheck_predicates,
        })
    }

    fn param_name(&self) -> &str {
        &self.data_param.name
    }
}

pub(crate) async fn map_argument<'a>(
    data_type: &'a MutationType,
    argument: &'a Val,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<(Vec<InsertionRow>, Vec<AbstractPredicate>), PostgresExecutionError> {
    match argument {
        Val::List(arguments) => {
            let mapped = arguments
                .iter()
                .map(|argument| map_single(data_type, argument, subsystem, request_context));
            let mapped: Vec<(InsertionRow, Vec<AbstractPredicate>)> = try_join_all(mapped).await?;

            let mut precheck_queries = vec![];
            let mut operations = vec![];

            for (insertion_row, precheck_predicates) in mapped {
                precheck_queries.extend(precheck_predicates);
                operations.push(insertion_row);
            }

            Ok((operations, precheck_queries))
        }
        _ => {
            let (insertion_row, precheck_predicates) =
                map_single(data_type, argument, subsystem, request_context).await?;
            Ok((vec![insertion_row], precheck_predicates))
        }
    }
}

/// Map a single item from the data parameter
#[async_recursion]
async fn map_single<'a>(
    data_type: &'a MutationType,
    argument: &'a Val,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<(InsertionRow, Vec<AbstractPredicate>), PostgresExecutionError> {
    let (precheck_predicate, _entity_predicate) = check_access(
        &subsystem.core_subsystem.entity_types[data_type.entity_id],
        &[],
        &SQLOperationKind::Create,
        subsystem,
        request_context,
        Some(&AccessInput {
            value: argument,
            ignore_missing_value: true,
            aliases: HashMap::new(),
        }),
    )
    .await?;

    use futures::stream::{self, StreamExt};

    let mapped = stream::iter(&data_type.fields)
        .map(|field| async move {
            let field_arg =
                postgres_core_resolver::predicate_util::get_argument_field(argument, &field.name);

            // If the argument has not been supplied, but has a default value, extract it from the context
            let field_arg = match field_arg {
                Some(_) => Ok(field_arg),
                None => {
                    if let Some(PostgresFieldDefaultValue::Dynamic(selection)) =
                        &field.default_value
                    {
                        subsystem
                            .core_subsystem
                            .extract_context_selection(request_context, selection)
                            .await
                    } else {
                        Ok(None)
                    }
                }
            }
            .ok()?;

            field_arg.map(|field_arg| async move {
                match &field.relation {
                    PostgresRelation::Scalar { column_id, .. } => {
                        vec![map_self_column(*column_id, field, field_arg, subsystem).await]
                    }

                    PostgresRelation::ManyToOne {
                        relation: ManyToOneRelation { relation_id, .. },
                        ..
                    } => {
                        let ManyToOne { column_pairs, .. } =
                            relation_id.deref(&subsystem.core_subsystem.database);

                        let mapped = column_pairs.iter().map(|column_pair| {
                            map_self_column(column_pair.self_column_id, field, field_arg, subsystem)
                        });
                        join_all(mapped).await
                    }

                    PostgresRelation::OneToMany(one_to_many_relation) => {
                        vec![
                            map_foreign(
                                field,
                                field_arg,
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
    let row: Result<Vec<InsertionElement>, PostgresExecutionError> =
        join_all(row).await.into_iter().flatten().collect();

    Ok((InsertionRow { elems: row? }, vec![precheck_predicate]))
}

async fn map_self_column<'a>(
    key_column_id: ColumnId,
    field: &'a PostgresField<MutationType>,
    argument: &'a Val,
    subsystem: &'a PostgresGraphQLSubsystem,
) -> Result<InsertionElement, PostgresExecutionError> {
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

            match postgres_core_resolver::predicate_util::get_argument_field(
                argument,
                foreign_type_pk_field_name,
            ) {
                Some(foreign_type_pk_arg) => foreign_type_pk_arg,
                None => {
                    // This can happen if we used a context value for a foreign key
                    // Instead of getting in the `{id: <value>}` format, we get the value directly
                    argument
                }
            }
        }
        _ => argument,
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
) -> Result<InsertionElement, PostgresExecutionError> {
    let field_type = base_type(
        &field.typ,
        &subsystem.core_subsystem.primitive_types,
        &subsystem.mutation_types,
    );

    let field_type = match field_type {
        PostgresType::Composite(field_type) => field_type,
        _ => unreachable!("Foreign type cannot be a primitive"), // TODO: Handle this at the type-level
    };

    let (insertions, precheck_predicates) =
        map_argument(field_type, argument, subsystem, request_context).await?;

    Ok(InsertionElement::NestedInsert(NestedInsertion {
        relation_id: one_to_many_relation.relation_id,
        insertions,
        precheck_predicates,
    }))
}
