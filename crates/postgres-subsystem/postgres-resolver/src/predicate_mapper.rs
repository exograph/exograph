use async_graphql_value::ConstValue;

use payas_sql::{AbstractPredicate, ColumnPath};
use postgres_model::{
    column_path::ColumnIdPath,
    model::ModelPostgresSystem,
    predicate::{PredicateParameter, PredicateParameterTypeKind},
};

use crate::{
    column_path_util::to_column_path,
    sql_mapper::{extract_and_map, SQLMapper},
    util::{get_argument_field, to_column_id_path, Arguments},
};

use super::{cast::cast_value, postgres_execution_error::PostgresExecutionError};

struct PredicateParamInput<'a> {
    pub param: &'a PredicateParameter,
    pub parent_column_path: Option<ColumnIdPath>,
}

impl<'a> SQLMapper<'a, AbstractPredicate<'a>> for PredicateParamInput<'a> {
    fn to_sql(
        self,
        argument: &'a ConstValue,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<AbstractPredicate<'a>, PostgresExecutionError> {
        let system = &subsystem;
        let parameter_type = &system.predicate_types[self.param.typ.type_id];

        match &parameter_type.kind {
            PredicateParameterTypeKind::ImplicitEqual => {
                let (op_key_path, op_value_path) =
                    operands(self.param, argument, self.parent_column_path, subsystem)?;

                Ok(AbstractPredicate::Eq(
                    op_key_path.into(),
                    op_value_path.into(),
                ))
            }
            PredicateParameterTypeKind::Operator(parameters) => {
                let predicate =
                    parameters
                        .iter()
                        .fold(AbstractPredicate::True, |acc, parameter| {
                            let arg = get_argument_field(argument, &parameter.name);
                            let new_predicate = match arg {
                                Some(op_value) => {
                                    let (op_key_column, op_value_column) = operands(
                                        self.param,
                                        op_value,
                                        self.parent_column_path.clone(),
                                        subsystem,
                                    )
                                    .expect("Could not get operands");
                                    AbstractPredicate::from_name(
                                        &parameter.name,
                                        op_key_column.into(),
                                        op_value_column.into(),
                                    )
                                }
                                None => AbstractPredicate::True,
                            };

                            AbstractPredicate::And(Box::new(acc), Box::new(new_predicate))
                        });

                Ok(predicate)
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
                            get_argument_field(argument, &parameter.name),
                        )
                    })
                    .fold(Ok(("", None)), |acc, (name, result)| {
                        acc.and_then(|(acc_name, acc_result)| {
                                    if acc_result.is_some() && result.is_some() {
                                        Err(PostgresExecutionError::Validation(self.param.name.to_string(), "Cannot specify more than one logical operation on the same level".into()))
                                    } else if acc_result.is_some() && result.is_none() {
                                        Ok((acc_name, acc_result))
                                    } else {
                                        Ok((name, result))
                                    }
                                })
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
                                        return Err(PostgresExecutionError::Validation(self.param.name.clone(), "Logical operation predicate does not have any arguments".into()));
                                    }

                                    // build our predicate chain from the array of arguments provided
                                    let identity_predicate = match logical_op_name {
                                        "and" => AbstractPredicate::True,
                                        "or" => AbstractPredicate::False,
                                        _ => todo!(),
                                    };

                                    let predicate_connector = match logical_op_name {
                                        "and" => AbstractPredicate::And,
                                        "or" => AbstractPredicate::Or,
                                        _ => todo!(),
                                    };

                                    let mut new_predicate = identity_predicate;

                                    for argument in arguments.iter() {
                                        let arg_predicate = PredicateParamInput {
                                            param: self.param,
                                            parent_column_path: self.parent_column_path.clone(),
                                        }
                                        .to_sql(argument, subsystem)?;

                                        new_predicate = predicate_connector(
                                            Box::new(new_predicate),
                                            Box::new(arg_predicate),
                                        );
                                    }

                                    Ok(new_predicate)
                                } else {
                                    Err(PostgresExecutionError::Validation(
                                        self.param.name.clone(),
                                        "This logical operation predicate needs a list of queries"
                                            .into(),
                                    ))
                                }
                            }

                            "not" => {
                                let arg_predicate = PredicateParamInput {
                                    param: self.param,
                                    parent_column_path: self.parent_column_path,
                                }
                                .to_sql(logical_op_argument_value, subsystem)?;

                                Ok(AbstractPredicate::Not(Box::new(arg_predicate)))
                            }

                            _ => todo!(),
                        }
                    }

                    _ => {
                        // we are dealing with field predicate arguments
                        // map field argument values into their respective predicates
                        let mut new_predicate = AbstractPredicate::True;

                        for parameter in field_params.iter() {
                            let arg = get_argument_field(argument, &parameter.name);

                            let new_column_path = to_column_id_path(
                                &self.parent_column_path,
                                &self.param.column_path_link,
                            );

                            let field_predicate = match arg {
                                Some(argument_value_component) => PredicateParamInput {
                                    param: parameter,
                                    parent_column_path: new_column_path,
                                }
                                .to_sql(argument_value_component, subsystem)?,
                                None => AbstractPredicate::True,
                            };

                            new_predicate = AbstractPredicate::And(
                                Box::new(new_predicate),
                                Box::new(field_predicate),
                            );
                        }

                        Ok(new_predicate)
                    }
                }
            }
        }
    }

    fn param_name(&self) -> &str {
        &self.param.name
    }
}

fn operands<'a>(
    param: &'a PredicateParameter,
    op_value: &'a ConstValue,
    parent_column_path: Option<ColumnIdPath>,
    subsystem: &'a ModelPostgresSystem,
) -> Result<(ColumnPath<'a>, ColumnPath<'a>), PostgresExecutionError> {
    let op_physical_column = param
        .column_path_link
        .as_ref()
        .expect("Could not find column path link while forming operands")
        .self_column_id
        .get_column(subsystem);
    let op_value = cast_value(op_value, &op_physical_column.typ);

    op_value
        .map(move |op_value| {
            (
                to_column_path(&parent_column_path, &param.column_path_link, subsystem),
                ColumnPath::Literal(op_value.unwrap().into()),
            )
        })
        .map_err(PostgresExecutionError::CastError)
}

pub fn compute_predicate<'a>(
    param: Option<&'a PredicateParameter>,
    arguments: &'a Arguments,
    subsystem: &'a ModelPostgresSystem,
) -> Result<AbstractPredicate<'a>, PostgresExecutionError> {
    extract_and_map(
        param.as_ref().map(|param| PredicateParamInput {
            param,
            parent_column_path: None,
        }),
        arguments,
        subsystem,
    )
    .map(|predicate| predicate.unwrap_or(AbstractPredicate::True))
}
