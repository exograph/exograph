use anyhow::*;
use async_graphql_value::Value;

use crate::{
    data::{
        create_data_param_mapper::InsertionInfo,
        mutation_resolver::{return_type_info, table_name},
    },
    sql::column::Column,
};

use payas_model::{
    model::{
        mapped_arena::SerializableSlabIndex,
        operation::{Mutation, UpdateDataParameter},
        relation::GqlRelation,
        types::GqlTypeKind,
        GqlCompositeTypeKind, GqlType,
    },
    sql::{
        column::PhysicalColumn, predicate::Predicate, transaction::TransactionScript, Cte,
        DynamicInsert, Insert, SQLOperation, Select, Update,
    },
};

use super::{
    operation_context::OperationContext,
    sql_mapper::{SQLMapper, SQLUpdateMapper},
};

impl<'a> SQLUpdateMapper<'a> for UpdateDataParameter {
    fn update_script(
        &'a self,
        mutation: &'a Mutation,
        predicate: &'a Predicate,
        select: Select<'a>,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<TransactionScript<'a>> {
        let system = &operation_context.query_context.system;
        let mutation_type = &system.mutation_types[self.type_id];

        let argument = match argument {
            Value::Variable(name) => operation_context.resolve_variable(name.as_str()).unwrap(),
            _ => argument,
        };

        let self_update_columns =
            compute_update_columns(mutation_type, argument, operation_context);

        let nested_updates = compute_nested(mutation_type, argument, operation_context);

        let (table, _, _) = return_type_info(mutation, operation_context);
        if nested_updates.is_empty() {
            let ops = vec![(
                table_name(mutation, operation_context),
                SQLOperation::Update(table.update(
                    self_update_columns,
                    predicate,
                    vec![operation_context.create_column(Column::Star)],
                )),
            )];
            Ok(TransactionScript::Single(SQLOperation::Cte(Cte {
                ctes: ops,
                select,
            })))
        } else {
            let pk_col = {
                let pk_physical_col = table.columns.iter().find(|col| col.is_pk).unwrap();
                operation_context.create_column(Column::Physical(pk_physical_col))
            };

            let update_op =
                SQLOperation::Update(table.update(self_update_columns, predicate, vec![pk_col]));

            let mut ops = vec![update_op];
            ops.extend(nested_updates);
            ops.push(SQLOperation::Select(select));
            Ok(TransactionScript::Multi(ops))
        }
    }
}

fn compute_update_columns<'a>(
    model_type: &'a GqlType,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> Vec<(&'a PhysicalColumn, &'a Column<'a>)> {
    let system = &operation_context.query_context.system;
    let argument = match argument {
        Value::Variable(name) => operation_context.resolve_variable(name.as_str()).unwrap(),
        _ => argument,
    };

    match &model_type.kind {
        GqlTypeKind::Primitive => panic!(),
        GqlTypeKind::Composite(GqlCompositeTypeKind { fields, .. }) => fields
            .iter()
            .flat_map(|field| {
                field.relation.self_column().and_then(|key_column_id| {
                    operation_context
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
                                    match operation_context.get_argument_field(
                                        argument_value,
                                        other_type_pk_field_name,
                                    ) {
                                        Some(other_type_pk_arg) => other_type_pk_arg,
                                        None => todo!(),
                                    }
                                }
                                _ => argument_value,
                            };

                            let value_column = operation_context
                                .literal_column(argument_value.clone(), key_column);
                            (key_column, value_column)
                        })
                })
            })
            .collect(),
    }
}

// A bit hacky way. Ideally, the nested parameter should have the same shape as the container type. Specifically, it should have
// the predicate parameter and the data parameter. Then we can simply use the same code that we use for the container type. That has
// an addtional advantage that the predicate can be more general ("where" in addition to the currently supported "id") so multiple objects
// can be updated at the same time.
// TODO: Do this once we rethink how we set up the parameters.
fn compute_nested<'a>(
    model_type: &'a GqlType,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> Vec<SQLOperation<'a>> {
    let system = &operation_context.query_context.system;

    match &model_type.kind {
        GqlTypeKind::Primitive => panic!(),
        GqlTypeKind::Composite(GqlCompositeTypeKind { fields, .. }) => {
            fields.iter().flat_map(|field| match &field.relation {
                GqlRelation::OneToMany { other_type_id, .. } => {
                    let field_model_type = &system.types[*other_type_id];
                    operation_context
                        .get_argument_field(argument, &field.name)
                        .iter()
                        .flat_map(|argument| {
                            let mut ops = vec![];
                            if let Some(op) = compute_nested_update(
                                field_model_type,
                                argument,
                                operation_context,
                                other_type_id,
                            ) {
                                ops.push(op);
                            }

                            ops.extend(compute_nested_create(
                                field_model_type,
                                argument,
                                operation_context,
                                other_type_id,
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

// Looks for the "update" field in the argument. If it exists, compute the SQLOperation needed to update the nested object.
fn compute_nested_update<'a>(
    field_model_type: &'a GqlType,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
    other_type_id: &SerializableSlabIndex<GqlType>,
) -> Option<SQLOperation<'a>> {
    operation_context
        .get_argument_field(argument, "update")
        .map(|update_argument| {
            let system = &operation_context.query_context.system;

            let nested =
                compute_update_columns(field_model_type, update_argument, operation_context);
            let (pk_columns, nested): (Vec<_>, Vec<_>) =
                nested.iter().partition(|elem| elem.0.is_pk);
            let predicate = pk_columns
                .iter()
                .fold(Predicate::True, |acc, (pk_col, value)| {
                    let pk_column = operation_context.create_column(Column::Physical(pk_col));
                    Predicate::And(Box::new(acc), Box::new(Predicate::Eq(pk_column, value)))
                });
            let other_type = &system.types[*other_type_id];
            let table = &system.tables[other_type.table_id().unwrap()];
            SQLOperation::Update(Update {
                table,
                predicate: operation_context.create_predicate(predicate),
                column_values: nested,
                returning: vec![],
            })
        })
}

// Looks for the "create" field in the argument. If it exists, compute the SQLOperation needed to create the nested object.
fn compute_nested_create<'a>(
    field_model_type: &'a GqlType,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
    _other_type_id: &SerializableSlabIndex<GqlType>,
) -> Vec<SQLOperation<'a>> {
    operation_context
        .get_argument_field(argument, "create")
        .map(|create_argument| {
            field_model_type
                .map_to_sql(create_argument, operation_context)
                .unwrap()
        })
        .map(|insertion_info| {
            insertion_info
                .operation(operation_context, false)
                .into_iter()
                .map(|(_, op)| op)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

// let returning = vec![];
// let x = table.insert(columns, values, returning);
// let x = {
//     let mut column_names = x.column_names;
//     //column_names.push("concert_id")
//     // DynamicInsert {
//     //     table: x.table,
//     //     column_names,
//     //     static_values: x.column_values_seq,
//     //     dynamic_values: vec![],

//     //     returning: (),
//     // }
// };
