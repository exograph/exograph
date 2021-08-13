use crate::sql::{column::Column, predicate::Predicate};
use async_graphql_value::Value::List;

use payas_model::model::predicate::*;

use async_graphql_value::Value;

use super::{operation_context::OperationContext, sql_mapper::SQLMapper};

impl<'a> SQLMapper<'a, Predicate<'a>> for PredicateParameter {
    fn map_to_sql(
        &'a self,
        argument_value: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> Predicate<'a> {
        let system = operation_context.query_context.system;
        let parameter_type = &system.predicate_types[self.type_id];

        let argument_value = match argument_value {
            Value::Variable(name) => operation_context.resolve_variable(name.as_str()).unwrap(),
            _ => argument_value,
        };

        match &parameter_type.kind {
            PredicateParameterTypeKind::ImplicitEqual => {
                let (op_key_column, op_value_column) =
                    operands(self, argument_value, operation_context);
                Predicate::Eq(op_key_column, op_value_column)
            }
            PredicateParameterTypeKind::Opeartor(parameters) => {
                parameters.iter().fold(Predicate::True, |acc, parameter| {
                    let arg = operation_context.get_argument_field(argument_value, &parameter.name);
                    let new_predicate = match arg {
                        Some(op_value) => {
                            let (op_key_column, op_value_column) =
                                operands(self, op_value, operation_context);
                            Predicate::from_name(&parameter.name, op_key_column, op_value_column)
                        }
                        None => Predicate::True,
                    };

                    Predicate::And(Box::new(acc), Box::new(new_predicate))
                })
            }
            PredicateParameterTypeKind::Composite(parameters, boolean_params) => {
                // first, match any boolean predicates the argument_value might contain
                let boolean_argument_value: (&str, Option<&Value>) = boolean_params
                    .iter()
                    .map(|parameter| {
                        (
                            parameter.name.as_str(),
                            operation_context.get_argument_field(argument_value, &parameter.name),
                        )
                    })
                    .fold(("", None), |(acc_name, acc_result), (name, result)| {
                        if acc_result.is_some() && result.is_some() {
                            panic!(
                                "Cannot specify more than one boolean predicate on the same level"
                            )
                        } else if acc_result.is_some() && result.is_none() {
                            (acc_name, acc_result)
                        } else {
                            (name, result)
                        }
                    });

                // do we have a match?
                match boolean_argument_value {
                    (boolean_predicate_name, Some(boolean_argument_value)) => {
                        // we have a single boolean predicate argument
                        // e.g. and: [..], or: [..], not: {..}

                        // we will now build a predicate from it

                        match boolean_predicate_name {
                            "and" | "or" => {
                                if let List(arguments) = boolean_argument_value {
                                    // first make sure we have arguments
                                    if arguments.is_empty() {
                                        panic!("Boolean predicate does not have any arguments")
                                    }

                                    // build our predicate chain from the array of arguments provided
                                    let identity_predicate = match boolean_predicate_name {
                                        "and" => Predicate::True,
                                        "or" => Predicate::False,
                                        _ => panic!(),
                                    };

                                    let predicate_connector = match boolean_predicate_name {
                                        "and" => |a, b| Predicate::And(Box::new(a), Box::new(b)),
                                        "or" => |a, b| Predicate::Or(Box::new(a), Box::new(b)),
                                        _ => panic!(),
                                    };

                                    arguments.iter().fold(identity_predicate, |acc, argument| {
                                        let mapped = self.map_to_sql(argument, operation_context);
                                        predicate_connector(acc, mapped)
                                    })
                                } else {
                                    panic!("This boolean predicate needs a list of queries")
                                }
                            }

                            "not" => Predicate::Not(Box::new(
                                self.map_to_sql(boolean_argument_value, operation_context),
                            )),

                            _ => panic!(),
                        }
                    }

                    _ => {
                        // we are dealing with field predicate arguments
                        // map field argument values into their respective predicates
                        parameters.iter().fold(Predicate::True, |acc, parameter| {
                            let arg = operation_context
                                .get_argument_field(argument_value, &parameter.name);
                            let new_predicate = match arg {
                                Some(argument_value_component) => parameter
                                    .map_to_sql(argument_value_component, operation_context),
                                None => Predicate::True,
                            };

                            Predicate::And(Box::new(acc), Box::new(new_predicate))
                        })
                    }
                }
            }
        }
    }
}

fn operands<'a>(
    param: &'a PredicateParameter,
    op_value: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> (&'a Column<'a>, &'a Column<'a>) {
    let system = &operation_context.query_context.system;
    let op_physical_column = &param.column_id.as_ref().unwrap().get_column(system);
    let op_key_column = operation_context.create_column(Column::Physical(op_physical_column));
    let op_value_column = operation_context.literal_column(op_value.clone(), op_physical_column);
    (op_key_column, op_value_column)
}
