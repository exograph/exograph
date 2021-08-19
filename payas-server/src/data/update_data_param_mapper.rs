use async_graphql_value::Value;

use crate::sql::column::Column;

use payas_model::{
    model::{
        operation::UpdateDataParameter, relation::GqlRelation, types::GqlTypeKind,
        GqlCompositeTypeKind, GqlType,
    },
    sql::{column::PhysicalColumn, predicate::Predicate, SQLOperation, Update},
};

use super::{operation_context::OperationContext, sql_mapper::SQLMapper};

pub struct MappedUpdateDataParameter<'a> {
    pub self_update_columns: Vec<(&'a PhysicalColumn, &'a Column<'a>)>,
    pub nested_updates: Vec<SQLOperation<'a>>,
}

impl<'a> SQLMapper<'a, MappedUpdateDataParameter<'a>> for UpdateDataParameter {
    fn map_to_sql(
        &'a self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> MappedUpdateDataParameter {
        let system = &operation_context.query_context.system;
        let model_type = &system.mutation_types[self.type_id];

        let argument = match argument {
            Value::Variable(name) => operation_context.resolve_variable(name.as_str()).unwrap(),
            _ => argument,
        };

        let self_update_columns = compute_update_columns(model_type, argument, operation_context);

        let nested_update = compute_nested_updates(model_type, argument, operation_context);

        MappedUpdateDataParameter {
            self_update_columns,
            nested_updates: nested_update,
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
fn compute_nested_updates<'a>(
    model_type: &'a GqlType,
    argument: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> Vec<SQLOperation<'a>> {
    let system = &operation_context.query_context.system;

    match &model_type.kind {
        GqlTypeKind::Primitive => panic!(),
        GqlTypeKind::Composite(GqlCompositeTypeKind { fields, .. }) => {
            fields.iter().filter_map(|field| match &field.relation {
                GqlRelation::OneToMany { other_type_id, .. } => {
                    let field_model_type = &system.types[*other_type_id];
                    operation_context
                        .get_argument_field(argument, &field.name)
                        .map(|argument| {
                            let argument = operation_context
                                .get_argument_field(argument, "update")
                                .unwrap();

                            let nested = compute_update_columns(
                                field_model_type,
                                argument,
                                operation_context,
                            );

                            let (pk_columns, nested): (Vec<_>, Vec<_>) =
                                nested.iter().partition(|elem| elem.0.is_pk);

                            // TODO: Add an additional predicate to ensure that this element is related to the containing entity
                            // For example, with "updateConcert", we need add a "concert_id" column
                            let predicate =
                                pk_columns
                                    .iter()
                                    .fold(Predicate::True, |acc, (pk_col, value)| {
                                        let pk_column = operation_context
                                            .create_column(Column::Physical(pk_col));
                                        Predicate::And(
                                            Box::new(acc),
                                            Box::new(Predicate::Eq(pk_column, value)),
                                        )
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
                _ => None,
            })
        }
    }
    .collect()
}
