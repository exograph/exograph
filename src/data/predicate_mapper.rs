use crate::sql::predicate::Predicate;
use crate::sql::table::PhysicalTable;

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
        let parameter_type = self.get_param_type(operation_context);

        match &parameter_type.kind {
            PredicateParameterTypeKind::Primitive => Predicate::Eq(
                table.get_column(&self.name).unwrap(),
                operation_context.literal_column(argument_value),
            ),
            PredicateParameterTypeKind::Composite {
                parameters,
                primitive_filter,
            } => parameters.iter().fold(Predicate::True, |acc, parameter| {
                let new_predicate =
                    match Self::get_argument_value_component(argument_value, &parameter.name) {
                        Some(argument_value_component) => {
                            if *primitive_filter {
                                parameter.op_predicate(
                                    &self.name,
                                    argument_value,
                                    table,
                                    operation_context,
                                )
                            } else {
                                parameter.predicate(
                                    argument_value_component,
                                    table,
                                    operation_context,
                                )
                            }
                        }
                        None => Predicate::True,
                    };

                Predicate::And(Box::new(acc), Box::new(new_predicate))
            }),
        }
    }

    fn op_predicate<'a>(
        &self,
        param_name: &str,
        argument_value: &'a Value,
        table: &'a PhysicalTable,
        operation_context: &'a OperationContext<'a>,
    ) -> Predicate<'a> {
        let parameter_type = self.get_param_type(operation_context);
        match &parameter_type.kind {
            PredicateParameterTypeKind::Primitive => {
                match Self::get_argument_value_component(argument_value, &self.name) {
                    Some(op_value) => {
                        let op_key_column = table.get_column(param_name).unwrap();
                        let op_value_column = operation_context.literal_column(op_value);
                        Predicate::from_name(&self.name, op_key_column, op_value_column)
                    }
                    None => Predicate::True,
                }
            }
            PredicateParameterTypeKind::Composite { .. } => todo!(),
        }
    }

    fn get_param_type<'a>(
        &self,
        operation_context: &'a OperationContext<'a>,
    ) -> &'a PredicateParameterType {
        operation_context
            .find_predicate_parameter_type(&self.type_name)
            .unwrap()
    }

    fn get_argument_value_component<'a>(
        argument_value: &'a Value,
        component_name: &str,
    ) -> Option<&'a Value> {
        match argument_value {
            Value::Object(value) => value.get(component_name),
            _ => None,
        }
    }
}
