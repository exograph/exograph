use async_graphql_value::ConstValue;

use payas_sql::{
    AbstractDelete, AbstractPredicate, AbstractSelect, AbstractUpdate, Column, ColumnPath,
    ColumnPathLink, NestedAbstractDelete, NestedAbstractInsert, NestedAbstractUpdate,
    NestedElementRelation, PhysicalColumn, PhysicalColumnType, Predicate, Selection,
};
use postgres_model::{
    model::ModelPostgresSystem,
    operation::{OperationReturnType, UpdateDataParameter},
    relation::PostgresRelation,
    types::{PostgresCompositeType, PostgresType, PostgresTypeKind},
};

use crate::util::{get_argument_field, return_type_info};

use super::{cast, postgres_execution_error::PostgresExecutionError, sql_mapper::SQLUpdateMapper};

impl<'a> SQLUpdateMapper<'a> for UpdateDataParameter {
    fn update_operation(
        &'a self,
        return_type: &'a OperationReturnType,
        predicate: AbstractPredicate<'a>,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<AbstractUpdate<'a>, PostgresExecutionError> {
        let data_type = &subsystem.mutation_types[self.type_id];

        let self_update_columns = compute_update_columns(data_type, argument, subsystem);
        let (table, _, _) = return_type_info(return_type, subsystem);

        let container_model_type = return_type.typ(subsystem);

        let (nested_updates, nested_inserts, nested_deletes) =
            compute_nested_ops(data_type, argument, container_model_type, subsystem);

        let abs_update = AbstractUpdate {
            table,
            predicate,
            column_values: self_update_columns,
            selection: select,
            nested_updates,
            nested_inserts,
            nested_deletes,
        };

        Ok(abs_update)
    }
}

fn compute_update_columns<'a>(
    data_type: &'a PostgresType,
    argument: &'a ConstValue,
    subsystem: &'a ModelPostgresSystem,
) -> Vec<(&'a PhysicalColumn, Column<'a>)> {
    match &data_type.kind {
        PostgresTypeKind::Primitive => panic!(),
        PostgresTypeKind::Composite(PostgresCompositeType { fields, .. }) => fields
            .iter()
            .flat_map(|field| {
                field.relation.self_column().and_then(|key_column_id| {
                    get_argument_field(argument, &field.name).map(|argument_value| {
                        let key_column = key_column_id.get_column(subsystem);
                        let argument_value = match &field.relation {
                            PostgresRelation::ManyToOne { other_type_id, .. } => {
                                let other_type = &subsystem.postgres_types[*other_type_id];
                                let other_type_pk_field_name = other_type
                                    .pk_column_id()
                                    .map(|column_id| &column_id.get_column(subsystem).column_name)
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
            .collect(),
    }
}

// A bit hacky way. Ideally, the nested parameter should have the same shape as the container type. Specifically, it should have
// the predicate parameter and the data parameter. Then we can simply use the same code that we use for the container type. That has
// an additional advantage that the predicate can be more general ("where" in addition to the currently supported "id") so multiple objects
// can be updated at the same time.
// TODO: Do this once we rethink how we set up the parameters.
fn compute_nested_ops<'a>(
    field_model_type: &'a PostgresType,
    argument: &'a ConstValue,
    container_model_type: &'a PostgresType,
    subsystem: &'a ModelPostgresSystem,
) -> (
    Vec<NestedAbstractUpdate<'a>>,
    Vec<NestedAbstractInsert<'a>>,
    Vec<NestedAbstractDelete<'a>>,
) {
    let mut nested_updates = vec![];
    let mut nested_inserts = vec![];
    let mut nested_deletes = vec![];

    match &field_model_type.kind {
        PostgresTypeKind::Primitive => {}
        PostgresTypeKind::Composite(PostgresCompositeType { fields, .. }) => {
            fields.iter().for_each(|field| {
                if let PostgresRelation::OneToMany { other_type_id, .. } = &field.relation {
                    let field_model_type = &subsystem.postgres_types[*other_type_id]; // TODO: This is a model type but should be a data type

                    if let Some(argument) = get_argument_field(argument, &field.name) {
                        nested_updates.extend(compute_nested_update(
                            field_model_type,
                            argument,
                            container_model_type,
                            subsystem,
                        ));

                        nested_inserts.extend(compute_nested_inserts(
                            field_model_type,
                            argument,
                            container_model_type,
                            subsystem,
                        ));

                        nested_deletes.extend(compute_nested_delete(
                            field_model_type,
                            argument,
                            subsystem,
                            container_model_type,
                        ));
                    }
                }
            })
        }
    }

    (nested_updates, nested_inserts, nested_deletes)
}

// Which column in field_model_type corresponds to the primary column in container_model_type?
fn compute_nested_reference_column<'a>(
    field_model_type: &'a PostgresType,
    container_model_type: &'a PostgresType,
    system: &'a ModelPostgresSystem,
) -> Option<&'a PhysicalColumn> {
    let pk_column = match &container_model_type.kind {
        PostgresTypeKind::Primitive => panic!(),
        PostgresTypeKind::Composite(kind) => {
            let container_table = &system.tables[kind.table_id];
            container_table.get_pk_physical_column()
        }
    }
    .unwrap();

    let nested_table = &system.tables[field_model_type.table_id().unwrap()];

    nested_table
        .columns
        .iter()
        .find(|column| match &column.typ {
            PhysicalColumnType::ColumnReference {
                ref_table_name,
                ref_column_name,
                ..
            } => {
                &pk_column.table_name == ref_table_name && &pk_column.column_name == ref_column_name
            }
            _ => false,
        })
}

// Look for the "update" field in the argument. If it exists, compute the SQLOperation needed to update the nested object.
fn compute_nested_update<'a>(
    field_model_type: &'a PostgresType,
    argument: &'a ConstValue,
    container_model_type: &'a PostgresType,
    subsystem: &'a ModelPostgresSystem,
) -> Vec<NestedAbstractUpdate<'a>> {
    let nested_reference_col =
        compute_nested_reference_column(field_model_type, container_model_type, subsystem).unwrap();

    let update_arg = get_argument_field(argument, "update");

    match update_arg {
        Some(update_arg) => match update_arg {
            arg @ ConstValue::Object(..) => {
                vec![compute_nested_update_object_arg(
                    field_model_type,
                    arg,
                    nested_reference_col,
                    subsystem,
                )]
            }
            ConstValue::List(update_arg) => update_arg
                .iter()
                .map(|arg| {
                    compute_nested_update_object_arg(
                        field_model_type,
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
    field_model_type: &'a PostgresType,
    argument: &'a ConstValue,
    nested_reference_col: &'a PhysicalColumn,
    subsystem: &'a ModelPostgresSystem,
) -> NestedAbstractUpdate<'a> {
    assert!(matches!(argument, ConstValue::Object(..)));

    let table = &subsystem.tables[field_model_type.table_id().unwrap()];

    let nested = compute_update_columns(field_model_type, argument, subsystem);
    let (pk_columns, nested): (Vec<_>, Vec<_>) = nested.into_iter().partition(|elem| elem.0.is_pk);

    // This computation of predicate based on the id column is not quite correct, but it is a flaw of how we let
    // mutation be specified. Currently (while performing abstract-sql refactoring), keeping the old behavior, but
    // will revisit it https://github.com/payalabs/payas/issues/376
    let predicate = pk_columns
        .into_iter()
        .fold(AbstractPredicate::True, |acc, (pk_col, value)| {
            let value = match value {
                Column::Literal(value) => ColumnPath::Literal(value),
                _ => panic!("Expected literal"),
            };
            AbstractPredicate::and(
                acc,
                AbstractPredicate::eq(
                    ColumnPath::Physical(vec![ColumnPathLink {
                        self_column: (pk_col, table),
                        linked_column: None,
                    }])
                    .into(),
                    value.into(),
                ),
            )
        });

    NestedAbstractUpdate {
        relation: payas_sql::NestedElementRelation {
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
                predicate: Predicate::True,
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
fn compute_nested_inserts<'a>(
    field_model_type: &'a PostgresType,
    argument: &'a ConstValue,
    container_model_type: &'a PostgresType,
    subsystem: &'a ModelPostgresSystem,
) -> Vec<NestedAbstractInsert<'a>> {
    fn create_nested<'a>(
        field_model_type: &'a PostgresType,
        argument: &'a ConstValue,
        container_model_type: &'a PostgresType,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<NestedAbstractInsert<'a>, PostgresExecutionError> {
        let nested_reference_col =
            compute_nested_reference_column(field_model_type, container_model_type, subsystem)
                .unwrap();

        let table = &subsystem.tables[field_model_type.table_id().unwrap()];

        let rows =
            super::create_data_param_mapper::map_argument(field_model_type, argument, subsystem)?;

        Ok(NestedAbstractInsert {
            relation: NestedElementRelation {
                column: nested_reference_col,
                table,
            },
            insert: payas_sql::AbstractInsert {
                table,
                rows,
                selection: AbstractSelect {
                    table,
                    selection: Selection::Seq(vec![]),
                    predicate: Predicate::True,
                    order_by: None,
                    offset: None,
                    limit: None,
                },
            },
        })
    }

    let create_arg = get_argument_field(argument, "create");

    match create_arg {
        Some(create_arg) => match create_arg {
            _arg @ ConstValue::Object(..) => vec![create_nested(
                field_model_type,
                create_arg,
                container_model_type,
                subsystem,
            )
            .unwrap()],
            ConstValue::List(create_arg) => create_arg
                .iter()
                .map(|arg| {
                    create_nested(field_model_type, arg, container_model_type, subsystem).unwrap()
                })
                .collect(),
            _ => panic!("Object or list expected"),
        },
        None => vec![],
    }
}

fn compute_nested_delete<'a>(
    field_model_type: &'a PostgresType,
    argument: &'a ConstValue,
    subsystem: &'a ModelPostgresSystem,
    container_model_type: &'a PostgresType,
) -> Vec<NestedAbstractDelete<'a>> {
    // This is not the right way. But current API needs to be updated to not even take the "id" parameter (the same issue exists in the "update" case).
    // TODO: Revisit this.

    let nested_reference_col =
        compute_nested_reference_column(field_model_type, container_model_type, subsystem).unwrap();

    let delete_arg = get_argument_field(argument, "delete");

    match delete_arg {
        Some(update_arg) => match update_arg {
            arg @ ConstValue::Object(..) => {
                vec![compute_nested_delete_object_arg(
                    field_model_type,
                    arg,
                    nested_reference_col,
                    subsystem,
                )]
            }
            ConstValue::List(update_arg) => update_arg
                .iter()
                .map(|arg| {
                    compute_nested_delete_object_arg(
                        field_model_type,
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
    field_model_type: &'a PostgresType,
    argument: &'a ConstValue,
    nested_reference_col: &'a PhysicalColumn,
    subsystem: &'a ModelPostgresSystem,
) -> NestedAbstractDelete<'a> {
    assert!(matches!(argument, ConstValue::Object(..)));

    let table = &subsystem.tables[field_model_type.table_id().unwrap()];

    //
    let nested = compute_update_columns(field_model_type, argument, subsystem);
    let (pk_columns, _nested): (Vec<_>, Vec<_>) = nested.into_iter().partition(|elem| elem.0.is_pk);

    // This computation of predicate based on the id column is not quite correct, but it is a flaw of how we let
    // mutation be specified. Currently (while performing abstract-sql refactoring), keeping the old behavior, but
    // will revisit it https://github.com/payalabs/payas/issues/376
    let predicate = pk_columns
        .into_iter()
        .fold(AbstractPredicate::True, |acc, (pk_col, value)| {
            let value = match value {
                Column::Literal(value) => ColumnPath::Literal(value),
                _ => panic!("Expected literal"),
            };
            AbstractPredicate::and(
                acc,
                AbstractPredicate::eq(
                    ColumnPath::Physical(vec![ColumnPathLink {
                        self_column: (pk_col, table),
                        linked_column: None,
                    }])
                    .into(),
                    value.into(),
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
                predicate: Predicate::True,
                order_by: None,
                offset: None,
                limit: None,
            },
        },
    }
}
