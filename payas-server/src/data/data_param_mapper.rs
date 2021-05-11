use async_graphql_value::Value;

use crate::{
    model::{operation::MutationDataParameter, relation::ModelRelation, types::ModelTypeKind},
    sql::column::Column,
};

use super::operation_context::OperationContext;

impl MutationDataParameter {
    pub fn compute_mutation_data<'a>(
        &'a self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> Vec<(&Column, &Column)> {
        let system = &operation_context.query_context.system;
        let model_type = &system.mutation_types[self.type_id];

        match &model_type.kind {
            ModelTypeKind::Primitive => panic!(),
            ModelTypeKind::Composite { fields, .. } => fields
                .iter()
                .flat_map(|field| {
                    field.relation.self_column().and_then(|key_column_id| {
                        super::get_argument_field(argument, &field.name).map(|argument_value| {
                            let key_physical_column = key_column_id.get_column(system);
                            let key_column = operation_context
                                .create_column(Column::Physical(&key_physical_column));
                            let argument_value = match &field.relation {
                                ModelRelation::ManyToOne { other_type_id, .. } => {
                                    let other_type = &system.types[*other_type_id];
                                    let other_type_pk_field_name = other_type
                                        .pk_column_id()
                                        .map(|column_id| &column_id.get_column(system).column_name)
                                        .unwrap();
                                    super::get_argument_field(
                                        argument_value,
                                        other_type_pk_field_name,
                                    )
                                    .unwrap()
                                }
                                _ => argument_value,
                            };
                            let value_column = operation_context
                                .literal_column(argument_value, key_physical_column);
                            (key_column, value_column)
                        })
                    })
                })
                .collect(),
        }
    }
}
