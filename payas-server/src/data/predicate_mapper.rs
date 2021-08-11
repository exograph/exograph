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
            PredicateParameterTypeKind::Composite(parameters, comparison_params) => {
                // generate_predicate_chain
                // generate a Predicate chain from a Vec<PredicateParameter>.
                //      for example, Predicate::And(Predicate::And(... Predicate::Eq(some, columns))) ...
                //
                // Pre:
                //      predicate_connector:
                //          a reference to a closure that 'connects' two Predicate values and returns the new connected Predicate
                //          for example: &|a, b| Predicate::Or(a, b)
                //      identity_predicate:
                //          this predicate should satisfy the property i <> x = x
                //          where i = identity_predicate (usually Predicate::True or Predicate::False)
                //                x = an arbitrary predicate
                //                <> = predicate_connector
                //          when evaluated with any predicate x
                //      parameters:
                //          a reference to a Vec<PredicateParameter> specifying column information
                //      argument_value:
                //          a reference to our current argument_value that we will pass recursively to map_to_sql()
                // Post: a chain of Predicates that will match
                let generate_predicate_chain =
                    |predicate_connector: &'a dyn Fn(
                        Box<Predicate<'a>>,
                        Box<Predicate<'a>>,
                    ) -> Predicate<'a>,
                     identity_predicate: Predicate<'a>,
                     parameters: &'a Vec<PredicateParameter>,
                     argument_value: &'a Value|
                     -> Predicate<'a> {
                        parameters
                            .iter()
                            .fold(identity_predicate.clone(), |acc, parameter| {
                                let arg = operation_context
                                    .get_argument_field(argument_value, &parameter.name);

                                let new_predicate = match arg {
                                    Some(argument_value_component) => parameter
                                        .map_to_sql(argument_value_component, operation_context),
                                    None => identity_predicate.clone(),
                                };

                                predicate_connector(Box::new(acc), Box::new(new_predicate))
                            })
                    };

                let (comparison_param_name, comparison_arg_value) = comparison_params.iter().fold(
                    ("", None),
                    |(acc_name, acc_value), comparison_param| {
                        let lookup_result = operation_context
                            .get_argument_field(argument_value, &comparison_param.name);

                        if lookup_result.is_some() && acc_value.is_some() {
                            panic!(
                                "Cannot specify both {} and {} at same level in query",
                                acc_name, &comparison_param.name
                            )
                        } else if lookup_result.is_some() {
                            (&comparison_param.name, lookup_result)
                        } else {
                            (acc_name, acc_value)
                        }
                    },
                );

                if let Some(List(clauses)) = comparison_arg_value {
                    let initial_predicate = match comparison_param_name {
                        "and" | "not" => Predicate::True,
                        "or" => Predicate::False,
                        _ => todo!("No such initial predicate implemented"),
                    };

                    let ret = clauses.iter().fold(initial_predicate, |acc, clause| {
                        match comparison_param_name {
                            "and" => {
                                let and_chain = generate_predicate_chain(
                                    &|a, b| Predicate::And(a, b),
                                    Predicate::True,
                                    parameters,
                                    clause,
                                );

                                Predicate::And(Box::new(acc), Box::new(and_chain))
                            }
                            "or" => {
                                let or_chain = generate_predicate_chain(
                                    &|a, b| Predicate::Or(a, b),
                                    Predicate::False,
                                    parameters,
                                    clause,
                                );

                                Predicate::Or(Box::new(acc), Box::new(or_chain))
                            }
                            "not" => {
                                // start with a regular Or chain
                                let or_chain = generate_predicate_chain(
                                    &|a, b| Predicate::Or(a, b),
                                    Predicate::False,
                                    parameters,
                                    clause,
                                );

                                // negate it (De Morgan's)
                                // (A | B)' = A' & B'
                                Predicate::And(
                                    Box::new(acc),
                                    Box::new(Predicate::Not(Box::new(or_chain))),
                                )
                            }
                            _ => todo!("Comparison predicate not implemented"),
                        }
                    });

                    // TODO: we generate an unwieldly structure above us:
                    //
                    // And(
                    //    And(
                    //        True,
                    //        Or(
                    //            Or(
                    //                Or(
                    //                    False,
                    //                    And(
                    //                        And(
                    //                            And(
                    //                                And(
                    //                                    And(
                    //                                        And(
                    //                                            True,
                    //                                            Eq(..
                    //
                    // don't know if this will affect performance too much, but it's something to
                    // improve later on
                    // see for yourself:
                    // eprintln!("{:#?}", ret);

                    return ret;
                }

                generate_predicate_chain(
                    &|a, b| Predicate::And(a, b),
                    Predicate::True,
                    parameters,
                    argument_value,
                )
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
