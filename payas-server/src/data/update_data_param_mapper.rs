use anyhow::*;
use async_graphql_value::Value;

use crate::sql::column::Column;

use payas_model::{
    model::{
        operation::UpdateDataParameter, relation::GqlRelation, types::GqlTypeKind,
        GqlCompositeTypeKind,
    },
    sql::column::PhysicalColumn,
};

use super::{operation_context::OperationContext, sql_mapper::SQLMapper};

impl<'a> SQLMapper<'a, Vec<(&'a PhysicalColumn, &'a Column<'a>)>> for UpdateDataParameter {
    fn map_to_sql(
        &'a self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<Vec<(&PhysicalColumn, &Column)>> {
        let system = &operation_context.query_context.system;
        let model_type = &system.mutation_types[self.type_id];

        let argument = match argument {
            Value::Variable(name) => operation_context.resolve_variable(name.as_str()).unwrap(),
            _ => argument,
        };

        let mapping = match &model_type.kind {
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
                                            .map(|column_id| {
                                                &column_id.get_column(system).column_name
                                            })
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
        };

        Ok(mapping)
    }
}
