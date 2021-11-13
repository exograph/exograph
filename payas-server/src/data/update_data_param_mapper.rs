use std::{cell::RefCell, rc::Rc};

use anyhow::*;
use async_graphql_value::ConstValue;
use maybe_owned::MaybeOwned;

use crate::{
    data::mutation_resolver::{return_type_info, table_name},
    execution::query_context::QueryContext,
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
        column::{PhysicalColumn, PhysicalColumnType, ProxyColumn},
        predicate::Predicate,
        transaction::{
            ConcreteTransactionStep, TemplateTransactionStep, TransactionScript, TransactionStep,
        },
        Cte, SQLOperation, Select, TemplateDelete, TemplateInsert, TemplateSQLOperation,
        TemplateUpdate,
    },
};

use super::operation_mapper::{SQLMapper, SQLUpdateMapper};

impl<'a> SQLUpdateMapper<'a> for UpdateDataParameter {
    fn update_script(
        &'a self,
        mutation: &'a Mutation,
        predicate: MaybeOwned<'a, Predicate<'a>>,
        select: Select<'a>,
        argument: &'a ConstValue,
        query_context: &'a QueryContext<'a>,
    ) -> Result<TransactionScript<'a>> {
        let system = &query_context.get_system();
        let data_type = &system.mutation_types[self.type_id];

        let self_update_columns = compute_update_columns(data_type, argument, query_context);

        let (table, _, _) = return_type_info(mutation, query_context);
        if !needs_transaction(data_type) {
            let ops = vec![(
                table_name(mutation, query_context),
                SQLOperation::Update(table.update(
                    self_update_columns,
                    predicate,
                    vec![Column::Star.into()],
                )),
            )];
            Ok(TransactionScript::Single(TransactionStep::Concrete(
                ConcreteTransactionStep::new(SQLOperation::Cte(Cte { ctes: ops, select })),
            )))
        } else {
            let pk_col = {
                let pk_physical_col = table.columns.iter().find(|col| col.is_pk).unwrap();
                Column::Physical(pk_physical_col).into()
            };

            let update_op = Rc::new(TransactionStep::Concrete(ConcreteTransactionStep::new(
                SQLOperation::Update(table.update(self_update_columns, predicate, vec![pk_col])),
            )));

            let container_model_type = mutation.return_type.typ(system);
            let nested_updates = compute_nested(
                data_type,
                argument,
                update_op.clone(),
                container_model_type,
                query_context,
            );

            let mut ops = vec![update_op.clone()];
            ops.extend(nested_updates.into_iter().map(Rc::new));

            Ok(TransactionScript::Multi(
                ops,
                TransactionStep::Concrete(ConcreteTransactionStep::new(SQLOperation::Select(
                    select,
                ))),
            ))
        }
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

fn needs_transaction(mutation_type: &GqlType) -> bool {
    match &mutation_type.kind {
        GqlTypeKind::Primitive => panic!(),
        GqlTypeKind::Composite(GqlCompositeType { fields, .. }) => fields
            .iter()
            .any(|field| matches!(&field.relation, GqlRelation::OneToMany { .. })),
    }
}

// A bit hacky way. Ideally, the nested parameter should have the same shape as the container type. Specifically, it should have
// the predicate parameter and the data parameter. Then we can simply use the same code that we use for the container type. That has
// an addtional advantage that the predicate can be more general ("where" in addition to the currently supported "id") so multiple objects
// can be updated at the same time.
// TODO: Do this once we rethink how we set up the parameters.
fn compute_nested<'a>(
    data_type: &'a GqlType,
    argument: &'a ConstValue,
    prev_step: Rc<TransactionStep<'a>>,
    container_model_type: &'a GqlType,
    query_context: &'a QueryContext<'a>,
) -> Vec<TransactionStep<'a>> {
    let system = &query_context.get_system();

    match &data_type.kind {
        GqlTypeKind::Primitive => panic!(),
        GqlTypeKind::Composite(GqlCompositeType { fields, .. }) => {
            fields.iter().flat_map(|field| match &field.relation {
                GqlRelation::OneToMany { other_type_id, .. } => {
                    let field_model_type = &system.types[*other_type_id]; // TODO: This is a model type but should be a data type
                    query_context
                        .get_argument_field(argument, &field.name)
                        .iter()
                        .flat_map(|argument| {
                            let mut ops = vec![];

                            ops.extend(compute_nested_update(
                                field_model_type,
                                argument,
                                query_context,
                                prev_step.clone(),
                                container_model_type,
                            ));

                            ops.extend(compute_nested_create(
                                field_model_type,
                                argument,
                                query_context,
                                prev_step.clone(),
                                container_model_type,
                            ));

                            ops.extend(compute_nested_delete(
                                field_model_type,
                                argument,
                                query_context,
                                prev_step.clone(),
                                container_model_type,
                            ));
                            ops
                        })
                        .collect()
                }
                _ => vec![],
            })
        }
    }
    .collect()
}

// Which column in field_model_type coresponds to the primary column in container_model_type?
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
    query_context: &'a QueryContext<'a>,
    prev_step: Rc<TransactionStep<'a>>,
    container_model_type: &'a GqlType,
) -> Vec<TransactionStep<'a>> {
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
                    query_context,
                    prev_step,
                    nested_reference_col,
                )]
            }
            ConstValue::List(update_arg) => update_arg
                .iter()
                .map(|arg| {
                    compute_nested_update_object_arg(
                        field_model_type,
                        arg,
                        query_context,
                        prev_step.clone(),
                        nested_reference_col,
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
    query_context: &'a QueryContext<'a>,
    prev_step: Rc<TransactionStep<'a>>,
    nested_reference_col: &'a PhysicalColumn,
) -> TransactionStep<'a> {
    assert!(matches!(argument, ConstValue::Object(..)));

    let system = &query_context.get_system();

    let nested = compute_update_columns(field_model_type, argument, query_context);
    let (pk_columns, nested): (Vec<_>, Vec<_>) = nested.into_iter().partition(|elem| elem.0.is_pk);

    let predicate = pk_columns
        .into_iter()
        .fold(Predicate::True, |acc, (pk_col, value)| {
            Predicate::and(
                acc,
                Predicate::Eq(Column::Physical(pk_col).into(), value.into()),
            )
        });
    let table = &system.tables[field_model_type.table_id().unwrap()];

    let mut nested_proxies: Vec<_> = nested
        .into_iter()
        .map(|(column, value)| (column, ProxyColumn::Concrete(value.into())))
        .collect();
    nested_proxies.push((
        nested_reference_col,
        ProxyColumn::Template {
            col_index: 0,
            step: prev_step.clone(),
        },
    ));

    let op = TemplateSQLOperation::Update(TemplateUpdate {
        table,
        predicate,
        column_values: nested_proxies,
        returning: vec![],
    });

    TransactionStep::Template(TemplateTransactionStep {
        operation: op,
        step: prev_step,
        values: RefCell::new(vec![]),
    })
}

// Looks for the "create" field in the argument. If it exists, compute the SQLOperation needed to create the nested object.
fn compute_nested_create<'a>(
    field_model_type: &'a GqlType,
    argument: &'a ConstValue,
    query_context: &'a QueryContext<'a>,
    prev_step: Rc<TransactionStep<'a>>,
    container_model_type: &'a GqlType,
) -> Vec<TransactionStep<'a>> {
    let system = &query_context.get_system();

    let step = query_context
        .get_argument_field(argument, "create")
        .map(|create_argument| {
            field_model_type
                .map_to_sql(create_argument, query_context)
                .unwrap()
        })
        .map(|insertion_info| {
            let nested_reference_col =
                compute_nested_reference_column(field_model_type, container_model_type, system)
                    .unwrap();
            let mut column_names = insertion_info.columns.clone();
            column_names.push(nested_reference_col);

            let column_values_seq: Vec<Vec<ProxyColumn>> = insertion_info
                .values
                .into_iter()
                .map(|subvalues| {
                    let mut proxied: Vec<_> =
                        subvalues.into_iter().map(ProxyColumn::Concrete).collect();
                    proxied.push(ProxyColumn::Template {
                        col_index: 0,
                        step: prev_step.clone(),
                    });
                    proxied
                })
                .collect();

            let op = TemplateSQLOperation::Insert(TemplateInsert {
                table: insertion_info.table,
                column_names,
                column_values_seq,
                returning: vec![],
            });

            TransactionStep::Template(TemplateTransactionStep {
                operation: op,
                step: prev_step,
                values: RefCell::new(vec![]),
            })
        });

    match step {
        Some(step) => vec![step],
        None => vec![],
    }
}

fn compute_nested_delete<'a>(
    field_model_type: &'a GqlType,
    argument: &'a ConstValue,
    query_context: &'a QueryContext<'a>,
    prev_step: Rc<TransactionStep<'a>>,
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
                step: prev_step,
                values: RefCell::new(vec![]),
            })]
        }
        None => vec![],
    }
}
