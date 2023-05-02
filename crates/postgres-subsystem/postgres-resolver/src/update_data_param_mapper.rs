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
use core_plugin_interface::core_resolver::context::RequestContext;
use core_plugin_interface::core_resolver::value::Val;
use exo_sql::{
    AbstractDelete, AbstractPredicate, AbstractSelect, AbstractUpdate, Column, ColumnPath,
    ColumnPathLink, NestedAbstractDelete, NestedAbstractInsert, NestedAbstractUpdate,
    NestedElementRelation, PhysicalColumn, PhysicalColumnType, Selection,
};
use futures::future::join_all;
use postgres_model::{
    mutation::DataParameter,
    relation::PostgresRelation,
    subsystem::PostgresSubsystem,
    types::{base_type, EntityType, MutationType, PostgresType, TypeIndex},
};

use crate::{
    sql_mapper::SQLMapper,
    util::{get_argument_field, return_type_info},
};

use super::{cast, postgres_execution_error::PostgresExecutionError};

pub struct UpdateOperation<'a> {
    pub data_param: &'a DataParameter,
    pub return_type: &'a OperationReturnType<EntityType>,
    pub predicate: AbstractPredicate<'a>,
    pub select: AbstractSelect<'a>,
}

#[async_trait]
impl<'a> SQLMapper<'a, AbstractUpdate<'a>> for UpdateOperation<'a> {
    async fn to_sql(
        self,
        argument: &'a Val,
        subsystem: &'a PostgresSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractUpdate<'a>, PostgresExecutionError> {
        let data_type = &subsystem.mutation_types[self.data_param.typ.innermost().type_id];

        let self_update_columns = compute_update_columns(data_type, argument, subsystem);
        let (table, _, _) = return_type_info(self.return_type, subsystem);

        let container_entity_type = self.return_type.typ(&subsystem.entity_types);

        let (nested_updates, nested_inserts, nested_deletes) = compute_nested_ops(
            data_type,
            argument,
            container_entity_type,
            subsystem,
            request_context,
        )
        .await;

        let abs_update = AbstractUpdate {
            table,
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
) -> Vec<(&'a PhysicalColumn, Column<'a>)> {
    data_type
        .fields
        .iter()
        .flat_map(|field| {
            field.relation.self_column().and_then(|key_column_id| {
                get_argument_field(argument, &field.name).map(|argument_value| {
                    let key_column = key_column_id.get_column(subsystem);
                    let argument_value = match &field.relation {
                        PostgresRelation::ManyToOne { other_type_id, .. } => {
                            let other_type = &subsystem.entity_types[*other_type_id];
                            let other_type_pk_field_name = other_type
                                .pk_column_id()
                                .map(|column_id| &column_id.get_column(subsystem).name)
                                .unwrap();
                            match get_argument_field(argument_value, other_type_pk_field_name) {
                                Some(other_type_pk_arg) => other_type_pk_arg,
                                None => todo!(),
                            }
                        }
                        _ => argument_value,
                    };

                    let value_column = cast::literal_column(argument_value, key_column);
                    (key_column, value_column.unwrap())
                })
            })
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
    container_entity_type: &'a EntityType,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> (
    Vec<NestedAbstractUpdate<'a>>,
    Vec<NestedAbstractInsert<'a>>,
    Vec<NestedAbstractDelete<'a>>,
) {
    let mut nested_updates = vec![];
    let mut nested_inserts = vec![];
    let mut nested_deletes = vec![];

    for field in arg_type.fields.iter() {
        if let PostgresRelation::OneToMany { .. } = &field.relation {
            let arg_type = match field.typ.innermost().type_id {
                TypeIndex::Primitive(_) => {
                    panic!("One to many relation should target a composite type")
                }
                TypeIndex::Composite(type_id) => &subsystem.mutation_types[type_id],
            };

            if let Some(argument) = get_argument_field(arg, &field.name) {
                nested_updates.extend(compute_nested_update(
                    arg_type,
                    argument,
                    container_entity_type,
                    subsystem,
                ));

                nested_inserts.extend(
                    compute_nested_inserts(
                        arg_type,
                        argument,
                        container_entity_type,
                        subsystem,
                        request_context,
                    )
                    .await,
                );

                nested_deletes.extend(compute_nested_delete(
                    arg_type,
                    argument,
                    subsystem,
                    container_entity_type,
                ));
            }
        }
    }

    (nested_updates, nested_inserts, nested_deletes)
}

// Which column in field_entity_type corresponds to the primary column in container_entity_type?
fn compute_nested_reference_column<'a>(
    field_entity_type: &'a MutationType,
    container_entity_type: &'a EntityType,
    system: &'a PostgresSubsystem,
) -> Option<&'a PhysicalColumn> {
    let pk_column = {
        let container_table = &system.tables[container_entity_type.table_id];
        container_table.get_pk_physical_column().unwrap()
    };

    let nested_table = &system.tables[system.entity_types[field_entity_type.entity_type].table_id];

    nested_table
        .columns
        .iter()
        .find(|column| match &column.typ {
            PhysicalColumnType::ColumnReference {
                ref_table_name,
                ref_column_name,
                ..
            } => &pk_column.table_name == ref_table_name && &pk_column.name == ref_column_name,
            _ => false,
        })
}

// Look for the "update" field in the argument. If it exists, compute the SQLOperation needed to update the nested object.
fn compute_nested_update<'a>(
    field_entity_type: &'a MutationType,
    argument: &'a Val,
    container_entity_type: &'a EntityType,
    subsystem: &'a PostgresSubsystem,
) -> Vec<NestedAbstractUpdate<'a>> {
    let nested_reference_col =
        compute_nested_reference_column(field_entity_type, container_entity_type, subsystem)
            .unwrap();

    let (update_arg, field_entity_type) =
        extract_argument(argument, field_entity_type, "update", subsystem);

    match update_arg {
        Some(update_arg) => match update_arg {
            arg @ Val::Object(..) => {
                vec![compute_nested_update_object_arg(
                    field_entity_type,
                    arg,
                    nested_reference_col,
                    subsystem,
                )]
            }
            Val::List(update_arg) => update_arg
                .iter()
                .map(|arg| {
                    compute_nested_update_object_arg(
                        field_entity_type,
                        arg,
                        nested_reference_col,
                        subsystem,
                    )
                })
                .collect(),
            _ => panic!("Object or list expected"),
        },
        None => vec![],
    }
}

// Compute update step assuming that the argument is a single object (not an array)
fn compute_nested_update_object_arg<'a>(
    field_entity_type: &'a MutationType,
    argument: &'a Val,
    nested_reference_col: &'a PhysicalColumn,
    subsystem: &'a PostgresSubsystem,
) -> NestedAbstractUpdate<'a> {
    assert!(matches!(argument, Val::Object(..)));

    let table = field_entity_type.table(subsystem);

    let nested = compute_update_columns(field_entity_type, argument, subsystem);
    let (pk_columns, nested): (Vec<_>, Vec<_>) = nested.into_iter().partition(|elem| elem.0.is_pk);

    // This computation of predicate based on the id column is not quite correct, but it is a flaw of how we let
    // mutation be specified. Currently (while performing abstract-sql refactoring), keeping the old behavior, but
    // will revisit it https://github.com/exograph/exograph/issues/376
    let predicate = pk_columns
        .into_iter()
        .fold(AbstractPredicate::True, |acc, (pk_col, value)| {
            let value = match value {
                Column::Param(value) => ColumnPath::Param(value),
                _ => panic!("Expected literal"),
            };
            AbstractPredicate::and(
                acc,
                AbstractPredicate::eq(
                    ColumnPath::Physical(vec![ColumnPathLink {
                        self_column: (pk_col, table),
                        linked_column: None,
                    }]),
                    value,
                ),
            )
        });

    NestedAbstractUpdate {
        relation: exo_sql::NestedElementRelation {
            column: nested_reference_col,
            table,
        },
        update: AbstractUpdate {
            table,
            predicate,
            column_values: nested,
            selection: AbstractSelect {
                table,
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
    }
}

// Looks for the "create" field in the argument. If it exists, compute the SQLOperation needed to create the nested object.
async fn compute_nested_inserts<'a>(
    field_entity_type: &'a MutationType,
    argument: &'a Val,
    container_entity_type: &'a EntityType,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Vec<NestedAbstractInsert<'a>> {
    async fn create_nested<'a>(
        field_entity_type: &'a MutationType,
        argument: &'a Val,
        container_entity_type: &'a EntityType,
        subsystem: &'a PostgresSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<NestedAbstractInsert<'a>, PostgresExecutionError> {
        let nested_reference_col =
            compute_nested_reference_column(field_entity_type, container_entity_type, subsystem)
                .unwrap();

        let table = field_entity_type.table(subsystem);

        let rows = super::create_data_param_mapper::map_argument(
            field_entity_type,
            argument,
            subsystem,
            request_context,
        )
        .await?;

        Ok(NestedAbstractInsert {
            relation: NestedElementRelation {
                column: nested_reference_col,
                table,
            },
            insert: exo_sql::AbstractInsert {
                table,
                rows,
                selection: AbstractSelect {
                    table,
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

    match create_arg {
        Some(create_arg) => match create_arg {
            _arg @ Val::Object(..) => vec![create_nested(
                field_entity_type,
                create_arg,
                container_entity_type,
                subsystem,
                request_context,
            )
            .await
            .unwrap()],
            Val::List(create_arg) => {
                join_all(create_arg.iter().map(|arg| async {
                    create_nested(
                        field_entity_type,
                        arg,
                        container_entity_type,
                        subsystem,
                        request_context,
                    )
                    .await
                    .unwrap()
                }))
                .await
            }
            _ => panic!("Object or list expected"),
        },
        None => vec![],
    }
}

fn compute_nested_delete<'a>(
    field_entity_type: &'a MutationType,
    argument: &'a Val,
    subsystem: &'a PostgresSubsystem,
    container_entity_type: &'a EntityType,
) -> Vec<NestedAbstractDelete<'a>> {
    // This is not the right way. But current API needs to be updated to not even take the "id" parameter (the same issue exists in the "update" case).
    // TODO: Revisit this.

    let nested_reference_col =
        compute_nested_reference_column(field_entity_type, container_entity_type, subsystem)
            .unwrap();

    let (delete_arg, field_entity_type) =
        extract_argument(argument, field_entity_type, "delete", subsystem);

    match delete_arg {
        Some(update_arg) => match update_arg {
            arg @ Val::Object(..) => {
                vec![compute_nested_delete_object_arg(
                    field_entity_type,
                    arg,
                    nested_reference_col,
                    subsystem,
                )]
            }
            Val::List(update_arg) => update_arg
                .iter()
                .map(|arg| {
                    compute_nested_delete_object_arg(
                        field_entity_type,
                        arg,
                        nested_reference_col,
                        subsystem,
                    )
                })
                .collect(),
            _ => panic!("Object or list expected"),
        },
        None => vec![],
    }
}

// Compute delete step assuming that the argument is a single object (not an array)
fn compute_nested_delete_object_arg<'a>(
    field_entity_type: &'a MutationType,
    argument: &'a Val,
    nested_reference_col: &'a PhysicalColumn,
    subsystem: &'a PostgresSubsystem,
) -> NestedAbstractDelete<'a> {
    assert!(matches!(argument, Val::Object(..)));

    let table = field_entity_type.table(subsystem);

    //
    let nested = compute_update_columns(field_entity_type, argument, subsystem);
    let (pk_columns, _nested): (Vec<_>, Vec<_>) = nested.into_iter().partition(|elem| elem.0.is_pk);

    // This computation of predicate based on the id column is not quite correct, but it is a flaw of how we let
    // mutation be specified. Currently (while performing abstract-sql refactoring), keeping the old behavior, but
    // will revisit it https://github.com/exograph/exograph/issues/376
    let predicate = pk_columns
        .into_iter()
        .fold(AbstractPredicate::True, |acc, (pk_col, value)| {
            let value = match value {
                Column::Param(value) => ColumnPath::Param(value),
                _ => panic!("Expected literal"),
            };
            AbstractPredicate::and(
                acc,
                AbstractPredicate::eq(
                    ColumnPath::Physical(vec![ColumnPathLink {
                        self_column: (pk_col, table),
                        linked_column: None,
                    }]),
                    value,
                ),
            )
        });

    NestedAbstractDelete {
        relation: NestedElementRelation {
            column: nested_reference_col,
            table,
        },
        delete: AbstractDelete {
            table,
            predicate,
            selection: AbstractSelect {
                table,
                selection: Selection::Seq(vec![]),
                predicate: AbstractPredicate::True,
                order_by: None,
                offset: None,
                limit: None,
            },
        },
    }
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
