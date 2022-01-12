use crate::{
    execution::query_context::QueryContext,
    sql::{column::Column, predicate::Predicate},
};
use anyhow::*;
use async_graphql_value::ConstValue;

use maybe_owned::MaybeOwned;
use payas_model::model::predicate::*;

pub trait PredicateParameterMapper<'a> {
    fn map_to_predicate(
        &'a self,
        argument_value: &'a ConstValue,
        query_context: &'a QueryContext<'a>,
        column_dependencies: &mut Vec<ColumnDependency>,
    ) -> Result<Predicate<'a>>;
}

impl<'a> PredicateParameterMapper<'a> for PredicateParameter {
    fn map_to_predicate(
        &'a self,
        argument_value: &'a ConstValue,
        query_context: &'a QueryContext<'a>,
        column_dependencies: &mut Vec<ColumnDependency>,
    ) -> Result<Predicate<'a>> {
        let system = query_context.get_system();
        let parameter_type = &system.predicate_types[self.type_id];

        match &parameter_type.kind {
            PredicateParameterTypeKind::ImplicitEqual => {
                let (op_key_column, op_value_column) =
                    operands(self, argument_value, column_dependencies, query_context);
                Ok(Predicate::Eq(op_key_column, op_value_column.into()))
            }
            PredicateParameterTypeKind::Operator(parameters) => {
                Ok(parameters.iter().fold(Predicate::True, |acc, parameter| {
                    let arg = query_context.get_argument_field(argument_value, &parameter.name);
                    let new_predicate = match arg {
                        Some(op_value) => {
                            let (op_key_column, op_value_column) =
                                operands(self, op_value, column_dependencies, query_context);
                            Predicate::from_name(
                                &parameter.name,
                                op_key_column,
                                op_value_column.into(),
                            )
                        }
                        None => Predicate::True,
                    };

                    Predicate::and(acc, new_predicate)
                }))
            }
            PredicateParameterTypeKind::Composite {
                field_params,
                logical_op_params,
            } => {
                // first, match any logical op predicates the argument_value might contain
                let logical_op_argument_value: (&str, Option<&ConstValue>) = logical_op_params
                    .iter()
                    .map(|parameter| {
                        (
                            parameter.name.as_str(),
                            query_context.get_argument_field(argument_value, &parameter.name),
                        )
                    })
                    .fold(Ok(("", None)), |acc, (name, result)| {
                        match acc {
                            Ok((acc_name, acc_result)) => {
                                if acc_result.is_some() && result.is_some() {
                                    bail!("Cannot specify more than one logical operation on the same level")
                                } else if acc_result.is_some() && result.is_none() {
                                    Ok((acc_name, acc_result))
                                } else {
                                    Ok((name, result))
                                }
                            },

                            err@Err(_) => err
                        }
                    })?;

                // do we have a match?
                match logical_op_argument_value {
                    (logical_op_name, Some(logical_op_argument_value)) => {
                        // we have a single logical op predicate argument
                        // e.g. and: [..], or: [..], not: {..}

                        // we will now build a predicate from it

                        match logical_op_name {
                            "and" | "or" => {
                                if let ConstValue::List(arguments) = logical_op_argument_value {
                                    // first make sure we have arguments
                                    if arguments.is_empty() {
                                        bail!("Logical operation predicate does not have any arguments")
                                    }

                                    // build our predicate chain from the array of arguments provided
                                    let identity_predicate = match logical_op_name {
                                        "and" => Predicate::True,
                                        "or" => Predicate::False,
                                        _ => todo!(),
                                    };

                                    let predicate_connector = match logical_op_name {
                                        "and" => Predicate::and,
                                        "or" => Predicate::or,
                                        _ => todo!(),
                                    };

                                    let mut new_predicate = identity_predicate;

                                    for argument in arguments.iter() {
                                        let mapped = self.map_to_predicate(
                                            argument,
                                            query_context,
                                            column_dependencies,
                                        )?;
                                        new_predicate = predicate_connector(new_predicate, mapped);
                                    }

                                    Ok(new_predicate)
                                } else {
                                    bail!(
                                        "This logical operation predicate needs a list of queries"
                                    )
                                }
                            }

                            "not" => Ok(Predicate::Not(Box::new(
                                self.map_to_predicate(
                                    logical_op_argument_value,
                                    query_context,
                                    column_dependencies,
                                )?
                                .into(),
                            ))),

                            _ => todo!(),
                        }
                    }

                    _ => {
                        // we are dealing with field predicate arguments
                        // map field argument values into their respective predicates
                        let mut new_predicate = Predicate::True;

                        for parameter in field_params.iter() {
                            let arg =
                                query_context.get_argument_field(argument_value, &parameter.name);
                            let mapped = match arg {
                                Some(argument_value_component) => parameter.map_to_predicate(
                                    argument_value_component,
                                    query_context,
                                    column_dependencies,
                                )?,
                                None => Predicate::True,
                            };

                            new_predicate = Predicate::and(new_predicate, mapped)
                        }

                        Ok(new_predicate)
                    }
                }
            }
        }
    }
}

fn operands<'a>(
    param: &'a PredicateParameter,
    op_value: &'a ConstValue,
    dependencies: &mut Vec<ColumnDependency>,
    query_context: &'a QueryContext<'a>,
) -> (MaybeOwned<'a, Column<'a>>, Column<'a>) {
    let system = query_context.get_system();

    if let Some(column_dependency) = param.column_dependency.as_ref() {
        dependencies.push(column_dependency.clone());
    };

    let op_physical_column = &param
        .column_dependency
        .as_ref()
        .unwrap()
        .column_id
        .get_column(system);
    let op_key_column = Column::Physical(op_physical_column).into();
    let op_value_column = query_context.literal_column(op_value, op_physical_column);
    (op_key_column, op_value_column.unwrap())
}
