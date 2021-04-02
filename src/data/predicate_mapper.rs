use crate::sql::table::PhysicalTable;
use crate::sql::{column::Column, predicate::Predicate};

use crate::model::predicate::*;

use async_graphql_value::Value;

use super::operation_context::OperationContext;

impl PredicateParameter {
    pub fn predicate<'a>(
        &self,
        argument_value: &'a Value,
        table: &'a PhysicalTable,
        operation_context: &'a OperationContext<'a>,
    ) -> Predicate<'a> {
        let parameter_type = operation_context
            .query_context
            .data_context
            .system
            .parameter_types
            .find_predicate_parameter_type(&self.type_name)
            .unwrap();

        match &parameter_type.kind {
            PredicateParameterTypeKind::Primitive => Predicate::Eq(
                table.get_column(&self.name).unwrap(),
                operation_context.literal_column(argument_value),
            ),
            PredicateParameterTypeKind::Composite {
                parameters,
                primitive_filter,
            } => parameters.iter().fold(Predicate::True, |acc, parameter| {
                let new_predicate = if *primitive_filter {
                    parameter.predicate(argument_value, table, operation_context)
                } else {
                    parameter.op_predicate(argument_value, table, operation_context)
                };
                Predicate::And(Box::new(acc), Box::new(new_predicate))
            }),
        }
    }

    fn op_predicate<'a>(
        &self,
        argument_value: &'a Value,
        table: &'a PhysicalTable,
        operation_context: &'a OperationContext<'a>,
    ) -> Predicate<'a> {
        match argument_value {
            Value::Object(value) => value.iter().fold(
                Predicate::True,
                |acc, (param_name, x_value)| match x_value {
                    Value::Object(value) => value.iter().fold(acc, |acc, (op_name, op_value)| {
                        let op_key_column = table.get_column(param_name).unwrap();
                        let op_value_column = operation_context.literal_column(op_value);
                        let new_predicate =
                            Predicate::from_name(op_name.as_str(), op_key_column, op_value_column);
                        Predicate::And(Box::new(acc), Box::new(new_predicate))
                    }),
                    _ => todo!(),
                },
            ),
            _ => todo!(),
        }
    }
}
