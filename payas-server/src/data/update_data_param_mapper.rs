use anyhow::Result;
use async_graphql_value::ConstValue;
use payas_sql::asql::{
    column_path::{ColumnPath, ColumnPathLink},
    predicate::AbstractPredicate,
    select::AbstractSelect,
    selection::{NestedElementRelation, Selection},
    update::{AbstractUpdate, NestedAbstractInsert, NestedAbstractUpdate},
};

use crate::{
    data::mutation_resolver::return_type_info, execution::query_context::QueryContext,
    sql::column::Column,
};

use payas_model::{
    model::{
        operation::{Mutation, UpdateDataParameter},
        relation::GqlRelation,
        system::ModelSystem,
        types::GqlTypeKind,
        GqlCompositeType, GqlType,
    },
    sql::{
        column::{PhysicalColumn, PhysicalColumnType},
        predicate::Predicate,
        transaction::{TemplateTransactionStep, TransactionStep, TransactionStepId},
        TemplateDelete, TemplateSQLOperation,
    },
};

use super::operation_mapper::SQLUpdateMapper;

impl<'a> SQLUpdateMapper<'a> for UpdateDataParameter {
    fn update_script(
        &'a self,
        mutation: &'a Mutation,
        predicate: AbstractPredicate<'a>,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        query_context: &'a QueryContext<'a>,
    ) -> Result<AbstractUpdate<'a>> {
        let system = &query_context.get_system();
        let data_type = &system.mutation_types[self.type_id];

        let self_update_columns = compute_update_columns(data_type, argument, query_context);
        let (table, _, _) = return_type_info(mutation, query_context);

        let container_model_type = mutation.return_type.typ(system);

        let (nested_updates, nested_inserts) =
            compute_nested_ops(data_type, argument, container_model_type, query_context);

        let abs_update = AbstractUpdate {
            table,
            predicate: predicate.into(),
            column_values: self_update_columns,
            selection: select,
            nested_update: nested_updates,
            nested_insert: nested_inserts,
        };

        Ok(abs_update)
    }
}

fn compute_update_columns<'a>(
    data_type: &'a GqlType,
    argument: &'a ConstValue,
    query_context: &'a QueryContext<'a>,
) -> Vec<(&'a PhysicalColumn, Column<'a>)> {
    let system = &query_context.get_system();

    match &data_type.kind {
        GqlTypeKind::Primitive => panic!(),
        GqlTypeKind::Composite(GqlCompositeType { fields, .. }) => fields
            .iter()
            .flat_map(|field| {
                field.relation.self_column().and_then(|key_column_id| {
                    query_context
                        .get_argument_field(argument, &field.name)
                        .map(|argument_value| {
                            let key_column = key_column_id.get_column(system);
                            let argument_value = match &field.relation {
                                GqlRelation::ManyToOne { other_type_id, .. } => {
                                    let other_type = &system.types[*other_type_id];
                                    let other_type_pk_field_name = other_type
                                        .pk_column_id()
                                        .map(|column_id| &column_id.get_column(system).column_name)
                                        .unwrap();
                                    match query_context.get_argument_field(
                                        argument_value,
                                        other_type_pk_field_name,
                                    ) {
                                        Some(other_type_pk_arg) => other_type_pk_arg,
                                        None => todo!(),
                                    }
                                }
                                _ => argument_value,
                            };

                            let value_column =
                                query_context.literal_column(argument_value, key_column);
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
    field_model_type: &'a GqlType,
    argument: &'a ConstValue,
    container_model_type: &'a GqlType,
    query_context: &'a QueryContext<'a>,
) -> (Vec<NestedAbstractUpdate<'a>>, Vec<NestedAbstractInsert<'a>>) {
    let system = &query_context.get_system();

    let mut nested_updates = vec![];
    let mut nested_inserts = vec![];

    match &field_model_type.kind {
        GqlTypeKind::Primitive => {}
        GqlTypeKind::Composite(GqlCompositeType { fields, .. }) => {
            fields.iter().for_each(|field| {
                if let GqlRelation::OneToMany { other_type_id, .. } = &field.relation {
                    let field_model_type = &system.types[*other_type_id]; // TODO: This is a model type but should be a data type

                    if let Some(argument) = query_context.get_argument_field(argument, &field.name)
                    {
                        nested_updates.extend(compute_nested_update(
                            field_model_type,
                            argument,
                            container_model_type,
                            query_context,
                        ));

                        nested_inserts.extend(compute_nested_inserts(
                            field_model_type,
                            argument,
                            container_model_type,
                            query_context,
                        ));

                        // ops.extend(compute_nested_delete(
                        //     field_model_type,
                        //     argument,
                        //     query_context,
                        //     prev_step_id,
                        //     container_model_type,
                        // ));
                    }
                }
            })
        }
    }

    (nested_updates, nested_inserts)
}

// Which column in field_model_type corresponds to the primary column in container_model_type?
fn compute_nested_reference_column<'a>(
    field_model_type: &'a GqlType,
    container_model_type: &'a GqlType,
    system: &'a ModelSystem,
) -> Option<&'a PhysicalColumn> {
    let pk_column = match &container_model_type.kind {
        GqlTypeKind::Primitive => panic!(),
        GqlTypeKind::Composite(kind) => {
            let container_table = &system.tables[kind.get_table_id()];
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

// Looks for the "update" field in the argument. If it exists, compute the SQLOperation needed to update the nested object.
fn compute_nested_update<'a>(
    field_model_type: &'a GqlType,
    argument: &'a ConstValue,
    container_model_type: &'a GqlType,
    query_context: &'a QueryContext<'a>,
) -> Vec<NestedAbstractUpdate<'a>> {
    let system = &query_context.get_system();

    let nested_reference_col =
        compute_nested_reference_column(field_model_type, container_model_type, system).unwrap();

    let update_arg = query_context.get_argument_field(argument, "update");

    match update_arg {
        Some(update_arg) => match update_arg {
            arg @ ConstValue::Object(..) => {
                vec![compute_nested_update_object_arg(
                    field_model_type,
                    arg,
                    nested_reference_col,
                    query_context,
                )]
            }
            ConstValue::List(update_arg) => update_arg
                .iter()
                .map(|arg| {
                    compute_nested_update_object_arg(
                        field_model_type,
                        arg,
                        nested_reference_col,
                        query_context,
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
    field_model_type: &'a GqlType,
    argument: &'a ConstValue,
    nested_reference_col: &'a PhysicalColumn,
    query_context: &'a QueryContext<'a>,
) -> NestedAbstractUpdate<'a> {
    assert!(matches!(argument, ConstValue::Object(..)));

    let system = &query_context.get_system();
    let table = &system.tables[field_model_type.table_id().unwrap()];

    let nested = compute_update_columns(field_model_type, argument, query_context);
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
        relation: payas_sql::asql::selection::NestedElementRelation {
            column: nested_reference_col,
            table,
        },
        update: AbstractUpdate {
            table,
            predicate: Some(predicate),
            column_values: nested,
            selection: AbstractSelect {
                table,
                selection: Selection::Seq(vec![]),
                predicate: None,
                order_by: None,
                offset: None,
                limit: None,
            },
            nested_update: vec![],
            nested_insert: vec![],
        },
    }
}

// Looks for the "create" field in the argument. If it exists, compute the SQLOperation needed to create the nested object.
fn compute_nested_inserts<'a>(
    field_model_type: &'a GqlType,
    argument: &'a ConstValue,
    container_model_type: &'a GqlType,
    query_context: &'a QueryContext<'a>,
) -> Vec<NestedAbstractInsert<'a>> {
    fn create_nested<'a>(
        field_model_type: &'a GqlType,
        argument: &'a ConstValue,
        container_model_type: &'a GqlType,
        query_context: &'a QueryContext<'a>,
    ) -> Result<NestedAbstractInsert<'a>> {
        let nested_reference_col = compute_nested_reference_column(
            field_model_type,
            container_model_type,
            query_context.get_system(),
        )
        .unwrap();
        let system = &query_context.get_system();

        let table = &system.tables[field_model_type.table_id().unwrap()];

        let rows = super::create_data_param_mapper::map_argument(
            field_model_type,
            argument,
            query_context,
        )?;

        Ok(NestedAbstractInsert {
            relation: NestedElementRelation {
                column: nested_reference_col,
                table,
            },
            insert: payas_sql::asql::insert::AbstractInsert {
                table,
                rows,
                selection: AbstractSelect {
                    table,
                    selection: Selection::Seq(vec![]),
                    predicate: None,
                    order_by: None,
                    offset: None,
                    limit: None,
                },
            },
        })
    }

    let create_arg = query_context.get_argument_field(argument, "create");
    println!("create_arg {:?}", argument);

    match create_arg {
        Some(create_arg) => match create_arg {
            _arg @ ConstValue::Object(..) => vec![create_nested(
                field_model_type,
                create_arg,
                container_model_type,
                query_context,
            )
            .unwrap()],
            ConstValue::List(create_arg) => create_arg
                .iter()
                .map(|arg| {
                    create_nested(field_model_type, arg, container_model_type, query_context)
                        .unwrap()
                })
                .collect(),
            _ => panic!("Object or list expected"),
        },
        None => vec![],
    }
}

fn compute_nested_delete<'a>(
    field_model_type: &'a GqlType,
    argument: &'a ConstValue,
    query_context: &'a QueryContext<'a>,
    prev_step_id: TransactionStepId,
    _container_model_type: &'a GqlType,
) -> Vec<TransactionStep<'a>> {
    // This is not the right way. But current API needs to be updated to not even take the "id" parameter (the same issue exists in the "update" case).
    // TODO: Revisit this.

    fn compute_predicate<'a>(
        elem_value: &ConstValue,
        field_model_type: &'a GqlType,
        query_context: &'a QueryContext<'a>,
    ) -> Predicate<'a> {
        let system = &query_context.get_system();

        let pk_field = field_model_type.pk_field().unwrap();

        match elem_value {
            ConstValue::Object(map) => {
                let pk_value = map.get(pk_field.name.as_str()).unwrap();
                let pk_column = field_model_type
                    .pk_column_id()
                    .map(|pk_column| pk_column.get_column(system))
                    .unwrap();

                Predicate::Eq(
                    Column::Physical(pk_column).into(),
                    query_context
                        .literal_column(pk_value, pk_column)
                        .unwrap()
                        .into(),
                )
            }
            ConstValue::List(values) => {
                let mut predicate = Predicate::False;
                for value in values {
                    let elem_predicate = compute_predicate(value, field_model_type, query_context);
                    predicate = Predicate::or(predicate, elem_predicate);
                }
                predicate
            }
            _ => panic!("Expected an object or a list"),
        }
    }

    let argument = query_context.get_argument_field(argument, "delete");

    match argument {
        Some(argument) => {
            let predicate = compute_predicate(argument, field_model_type, query_context);
            let system = &query_context.get_system();
            vec![TransactionStep::Template(TemplateTransactionStep {
                operation: TemplateSQLOperation::Delete(TemplateDelete {
                    table: &system.tables[field_model_type.table_id().unwrap()],
                    predicate,
                    returning: vec![],
                }),
                prev_step_id,
            })]
        }
        None => vec![],
    }
}
