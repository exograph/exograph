use crate::sql::{column::Column, predicate::Predicate};

use crate::model::predicate::*;

use async_graphql_value::Value;

use super::operation_context::OperationContext;

impl PredicateParameter {
    pub fn compute_predicate<'a>(
        &self,
        argument_value: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> Predicate<'a> {
        let system = operation_context.query_context.system;
        let parameter_type = &system.predicate_types[self.type_id];

        match &parameter_type.kind {
            PredicateParameterTypeKind::ImplicitEqual => {
                let (op_key_column, op_value_column) =
                    self.operands(argument_value, operation_context);
                Predicate::Eq(op_key_column, op_value_column)
            }
            PredicateParameterTypeKind::Opeartor(parameters) => {
                parameters.iter().fold(Predicate::True, |acc, parameter| {
                    let new_predicate =
                        match super::get_argument_field(argument_value, &parameter.name) {
                            Some(op_value) => {
                                let (op_key_column, op_value_column) =
                                    self.operands(op_value, operation_context);
                                Predicate::from_name(
                                    &parameter.name,
                                    op_key_column,
                                    op_value_column,
                                )
                            }
                            None => Predicate::True,
                        };

                    Predicate::And(Box::new(acc), Box::new(new_predicate))
                })
            }
            PredicateParameterTypeKind::Composite(parameters) => {
                parameters.iter().fold(Predicate::True, |acc, parameter| {
                    let new_predicate =
                        match super::get_argument_field(argument_value, &parameter.name) {
                            Some(argument_value_component) => parameter
                                .compute_predicate(argument_value_component, operation_context),
                            None => Predicate::True,
                        };

                    Predicate::And(Box::new(acc), Box::new(new_predicate))
                })
            }
        }
    }

    fn operands<'a>(
        &self,
        op_value: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> (&'a Column<'a>, &'a Column<'a>) {
        let system = &operation_context.query_context.system;
        let op_physical_column = &self.column_id.as_ref().unwrap().get_column(system);
        let op_key_column = operation_context.create_column(Column::Physical(op_physical_column));
        let op_value_column = operation_context.literal_column(op_value, op_physical_column);
        (op_key_column, op_value_column)
    }
}
