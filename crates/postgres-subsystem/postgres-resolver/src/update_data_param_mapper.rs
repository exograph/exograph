// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use core_plugin_interface::core_model::types::OperationReturnType;
use core_plugin_interface::core_resolver::access_solver::AccessSolver;
use core_plugin_interface::core_resolver::context::RequestContext;
use core_plugin_interface::core_resolver::value::Val;
use exo_sql::{
    AbstractDelete, AbstractInsert, AbstractPredicate, AbstractSelect, AbstractUpdate, Column,
    ColumnId, ColumnPath, ManyToOne, NestedAbstractDelete, NestedAbstractInsert,
    NestedAbstractInsertSet, NestedAbstractUpdate, OneToMany, PhysicalColumnPath, Selection,
};
use futures::StreamExt;
use postgres_model::{
    mutation::DataParameter,
    relation::{ManyToOneRelation, OneToManyRelation, PostgresRelation},
    subsystem::PostgresSubsystem,
    types::{base_type, EntityType, MutationType, PostgresType, TypeIndex},
};

use crate::{
    auth_util::check_access,
    sql_mapper::{SQLMapper, SQLOperationKind},
    util::{get_argument_field, return_type_info},
};

use super::{cast, postgres_execution_error::PostgresExecutionError};

pub struct UpdateOperation<'a> {
    pub data_param: &'a DataParameter,
    pub return_type: &'a OperationReturnType<EntityType>,
    pub predicate: AbstractPredicate,
    pub select: AbstractSelect,
}

#[async_trait]
impl<'a> SQLMapper<'a, AbstractUpdate> for UpdateOperation<'a> {
    async fn to_sql(
        self,
        argument: &'a Val,
        subsystem: &'a PostgresSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractUpdate, PostgresExecutionError> {
        let data_type = &subsystem.mutation_types[self.data_param.typ.innermost().type_id];

        let self_update_columns = compute_update_columns(data_type, argument, subsystem);
        let (table_id, _, _) = return_type_info(self.return_type, subsystem);

        let (nested_updates, nested_inserts, nested_deletes) =
            compute_nested_ops(data_type, argument, subsystem, request_context).await?;

        let abs_update = AbstractUpdate {
            table_id,
            predicate: self.predicate,
            column_values: self_update_columns,
            selection: self.select,
            nested_updates,
            nested_inserts,
            nested_deletes,
        };

        Ok(abs_update)
    }

    fn param_name(&self) -> &str {
        &self.data_param.name
    }
}

fn compute_update_columns<'a>(
    data_type: &'a MutationType,
    argument: &'a Val,
    subsystem: &'a PostgresSubsystem,
) -> Vec<(ColumnId, Column)> {
    data_type
        .fields
        .iter()
        .flat_map(|field| match &field.relation {
            PostgresRelation::Pk { column_id } | PostgresRelation::Scalar { column_id } => {
                get_argument_field(argument, &field.name).map(|argument_value| {
                    let column = column_id.get_column(&subsystem.database);
                    let value_column = cast::literal_column(argument_value, column);
                    (*column_id, value_column.unwrap())
                })
            }
            PostgresRelation::ManyToOne(ManyToOneRelation {
                foreign_pk_field_id,
                relation_id,
                ..
            }) => {
                let ManyToOne { self_column_id, .. } = relation_id.deref(&subsystem.database);

                let self_column = self_column_id.get_column(&subsystem.database);
                let foreign_type_pk_field_name =
                    &foreign_pk_field_id.resolve(&subsystem.entity_types).name;
                get_argument_field(argument, &field.name).map(|argument_value| {
                    match get_argument_field(argument_value, foreign_type_pk_field_name) {
                        Some(foreign_type_pk_arg) => {
                            let value_column =
                                cast::literal_column(foreign_type_pk_arg, self_column);
                            (self_column_id, value_column.unwrap())
                        }
                        None => unreachable!("Expected pk argument"), // Validation should have caught this
                    }
                })
            }
            PostgresRelation::OneToMany { .. } => None,
        })
        .collect()
}

// A bit hacky way. Ideally, the nested parameter should have the same shape as the container type. Specifically, it should have
// the predicate parameter and the data parameter. Then we can simply use the same code that we use for the container type. That has
// an additional advantage that the predicate can be more general ("where" in addition to the currently supported "id") so multiple objects
// can be updated at the same time.
// TODO: Do this once we rethink how we set up the parameters.
async fn compute_nested_ops<'a>(
    arg_type: &'a MutationType,
    arg: &'a Val,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<
    (
        Vec<NestedAbstractUpdate>,
        Vec<NestedAbstractInsertSet>,
        Vec<NestedAbstractDelete>,
    ),
    PostgresExecutionError,
> {
    let mut nested_updates = vec![];
    let mut nested_insert_sets = vec![];
    let mut nested_deletes = vec![];

    for field in arg_type.fields.iter() {
        if let PostgresRelation::OneToMany(OneToManyRelation { relation_id, .. }) = &field.relation
        {
            let nested_relation = &relation_id.deref(&subsystem.database);

            let arg_type = match field.typ.innermost().type_id {
                TypeIndex::Primitive(_) => {
                    // TODO: Fix this at the type-level
                    unreachable!("One to many relation should target a composite type")
                }
                TypeIndex::Composite(type_id) => &subsystem.mutation_types[type_id],
            };

            if let Some(argument) = get_argument_field(arg, &field.name) {
                nested_updates.extend(
                    compute_nested_update(
                        arg_type,
                        argument,
                        nested_relation,
                        subsystem,
                        request_context,
                    )
                    .await?,
                );

                nested_insert_sets.push(
                    compute_nested_inserts(
                        arg_type,
                        argument,
                        nested_relation,
                        subsystem,
                        request_context,
                    )
                    .await?,
                );

                nested_deletes.extend(
                    compute_nested_delete(
                        arg_type,
                        argument,
                        nested_relation,
                        subsystem,
                        request_context,
                    )
                    .await?,
                );
            }
        }
    }

    Ok((nested_updates, nested_insert_sets, nested_deletes))
}

// Look for the "update" field in the argument. If it exists, compute the SQLOperation needed to update the nested object.
async fn compute_nested_update<'a>(
    field_entity_type: &'a MutationType,
    argument: &'a Val,
    nesting_relation: &OneToMany,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<Vec<NestedAbstractUpdate>, PostgresExecutionError> {
    let (update_arg, field_entity_type) =
        extract_argument(argument, field_entity_type, "update", subsystem);

    match update_arg {
        Some(update_arg) => match update_arg {
            arg @ Val::Object(..) => Ok(vec![
                compute_nested_update_object_arg(
                    field_entity_type,
                    arg,
                    nesting_relation,
                    subsystem,
                    request_context,
                )
                .await?,
            ]),
            Val::List(update_arg) => futures::stream::iter(update_arg.iter())
                .then(|arg| async {
                    compute_nested_update_object_arg(
                        field_entity_type,
                        arg,
                        nesting_relation,
                        subsystem,
                        request_context,
                    )
                    .await
                })
                .collect::<Vec<Result<_, _>>>()
                .await
                .into_iter()
                .collect(),
            _ => panic!("Object or list expected"),
        },
        None => Ok(vec![]),
    }
}

// Compute update step assuming that the argument is a single object (not an array)
async fn compute_nested_update_object_arg<'a>(
    field_entity_type: &'a MutationType,
    argument: &'a Val,
    nesting_relation: &OneToMany,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<NestedAbstractUpdate, PostgresExecutionError> {
    assert!(matches!(argument, Val::Object(..)));

    let access_predicate = check_access(
        &subsystem.entity_types[field_entity_type.entity_id],
        &[],
        &SQLOperationKind::Update,
        subsystem,
        request_context,
        Some(argument),
    )
    .await?;

    let table_id = subsystem.entity_types[field_entity_type.entity_id].table_id;

    let nested = compute_update_columns(field_entity_type, argument, subsystem);
    let (pk_columns, nested): (Vec<_>, Vec<_>) = nested.into_iter().partition(|elem| {
        let column = elem.0.get_column(&subsystem.database);
        column.is_pk
    });

    // This computation of predicate based on the id column is not quite correct, but it is a flaw of how we let
    // mutation be specified. Currently (while performing abstract-sql refactoring), keeping the old behavior, but
    // will revisit it https://github.com/exograph/exograph/issues/376
    let arg_predicate =
        pk_columns
            .into_iter()
            .fold(AbstractPredicate::True, |acc, (pk_col, value)| {
                let value = match value {
                    Column::Param(value) => ColumnPath::Param(value),
                    _ => panic!("Expected literal"),
                };
                AbstractPredicate::and(
                    acc,
                    AbstractPredicate::eq(
                        ColumnPath::Physical(PhysicalColumnPath::leaf(pk_col)),
                        value,
                    ),
                )
            });

    let predicate = AbstractPredicate::and(arg_predicate, access_predicate);

    Ok(NestedAbstractUpdate {
        nesting_relation: *nesting_relation,
        update: AbstractUpdate {
            table_id,
            predicate,
            column_values: nested,
            selection: AbstractSelect {
                table_id,
                selection: Selection::Seq(vec![]),
                predicate: AbstractPredicate::True,
                order_by: None,
                offset: None,
                limit: None,
            },
            nested_updates: vec![],
            nested_inserts: vec![],
            nested_deletes: vec![],
        },
    })
}

// Looks for the "create" field in the argument. If it exists, compute the SQLOperation needed to create the nested object.
async fn compute_nested_inserts<'a>(
    field_entity_type: &'a MutationType,
    argument: &'a Val,
    nesting_relation: &OneToMany,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<NestedAbstractInsertSet, PostgresExecutionError> {
    async fn create_nested<'a>(
        field_entity_type: &'a MutationType,
        argument: &'a Val,
        nesting_relation: &OneToMany,
        subsystem: &'a PostgresSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<NestedAbstractInsert, PostgresExecutionError> {
        let table_id = subsystem.entity_types[field_entity_type.entity_id].table_id;

        let rows = super::create_data_param_mapper::map_argument(
            field_entity_type,
            argument,
            subsystem,
            request_context,
        )
        .await?;

        Ok(NestedAbstractInsert {
            relation_column_id: nesting_relation.foreign_column_id,
            insert: AbstractInsert {
                table_id,
                rows,
                selection: AbstractSelect {
                    table_id,
                    selection: Selection::Seq(vec![]),
                    predicate: AbstractPredicate::True,
                    order_by: None,
                    offset: None,
                    limit: None,
                },
            },
        })
    }

    let (create_arg, field_entity_type) =
        extract_argument(argument, field_entity_type, "create", subsystem);

    let inserts = match create_arg {
        Some(create_arg) => match create_arg {
            Val::Object(..) => Ok(vec![
                create_nested(
                    field_entity_type,
                    create_arg,
                    nesting_relation,
                    subsystem,
                    request_context,
                )
                .await?,
            ]),
            Val::List(create_arg) => futures::stream::iter(create_arg.iter())
                .then(|arg| async {
                    create_nested(
                        field_entity_type,
                        arg,
                        nesting_relation,
                        subsystem,
                        request_context,
                    )
                    .await
                })
                .collect::<Vec<Result<_, _>>>()
                .await
                .into_iter()
                .collect(),
            _ => panic!("Object or list expected"),
        },
        None => Ok(vec![]),
    }?;

    let access_predicate = match field_entity_type.database_access {
        Some(access_expr_index) => subsystem
            .solve(
                request_context,
                None,
                &subsystem.database_access_expressions[access_expr_index],
            )
            .await?
            .map(|expr| expr.0)
            .unwrap_or(AbstractPredicate::True),
        None => AbstractPredicate::True,
    };

    Ok(NestedAbstractInsertSet::new(inserts, access_predicate))
}

async fn compute_nested_delete<'a>(
    field_entity_type: &'a MutationType,
    argument: &'a Val,
    nesting_relation: &OneToMany,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<Vec<NestedAbstractDelete>, PostgresExecutionError> {
    // This is not the right way. But current API needs to be updated to not even take the "id" parameter (the same issue exists in the "update" case).
    // TODO: Revisit this.

    let (delete_arg, field_entity_type) =
        extract_argument(argument, field_entity_type, "delete", subsystem);

    match delete_arg {
        Some(delete_arg) => match delete_arg {
            arg @ Val::Object(..) => Ok(vec![
                compute_nested_delete_object_arg(
                    field_entity_type,
                    arg,
                    nesting_relation,
                    subsystem,
                    request_context,
                )
                .await?,
            ]),
            Val::List(delete_arg) => futures::stream::iter(delete_arg.iter())
                .then(|arg| async {
                    compute_nested_delete_object_arg(
                        field_entity_type,
                        arg,
                        nesting_relation,
                        subsystem,
                        request_context,
                    )
                    .await
                })
                .collect::<Vec<Result<_, _>>>()
                .await
                .into_iter()
                .collect(),
            _ => panic!("Object or list expected"),
        },
        None => Ok(vec![]),
    }
}

// Compute delete step assuming that the argument is a single object (not an array)
async fn compute_nested_delete_object_arg<'a>(
    field_mutation_type: &'a MutationType,
    argument: &'a Val,
    nesting_relation: &OneToMany,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<NestedAbstractDelete, PostgresExecutionError> {
    assert!(matches!(argument, Val::Object(..)));

    let nested = compute_update_columns(field_mutation_type, argument, subsystem);
    let (pk_columns, _nested): (Vec<_>, Vec<_>) = nested.into_iter().partition(|elem| {
        let column = elem.0.get_column(&subsystem.database);
        column.is_pk
    });

    let access_predicate = check_access(
        &subsystem.entity_types[field_mutation_type.entity_id],
        &[],
        &SQLOperationKind::Delete,
        subsystem,
        request_context,
        Some(argument),
    )
    .await?;

    // This computation of predicate based on the id column is not quite correct, but it is a flaw of how we let
    // mutation be specified. Currently (while performing abstract-sql refactoring), keeping the old behavior, but
    // will revisit it https://github.com/exograph/exograph/issues/376
    let arg_predicate =
        pk_columns
            .into_iter()
            .fold(AbstractPredicate::True, |acc, (pk_col, value)| {
                let value = match value {
                    Column::Param(value) => ColumnPath::Param(value),
                    _ => panic!("Expected literal"),
                };
                AbstractPredicate::and(
                    acc,
                    AbstractPredicate::eq(
                        ColumnPath::Physical(PhysicalColumnPath::leaf(pk_col)),
                        value,
                    ),
                )
            });

    let predicate = AbstractPredicate::and(arg_predicate, access_predicate);

    let table_id = subsystem.entity_types[field_mutation_type.entity_id].table_id;

    Ok(NestedAbstractDelete {
        nesting_relation: *nesting_relation,
        delete: AbstractDelete {
            table_id,
            predicate,
            selection: AbstractSelect {
                table_id,
                selection: Selection::Seq(vec![]),
                predicate: AbstractPredicate::True,
                order_by: None,
                offset: None,
                limit: None,
            },
        },
    })
}

fn extract_argument<'a>(
    argument: &'a Val,
    arg_type: &'a MutationType,
    arg_name: &str,
    subsystem: &'a PostgresSubsystem,
) -> (Option<&'a Val>, &'a MutationType) {
    let arg = get_argument_field(argument, arg_name);

    let arg_type = match base_type(
        &arg_type
            .fields
            .iter()
            .find(|f| f.name == arg_name)
            .unwrap()
            .typ,
        &subsystem.primitive_types,
        &subsystem.mutation_types,
    ) {
        PostgresType::Primitive(_) => panic!("{arg_name} argument type must be a composite type"),
        PostgresType::Composite(typ) => typ,
    };

    (arg, arg_type)
}
