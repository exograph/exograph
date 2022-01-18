use crate::{
    execution::query_context::QueryContext,
    sql::{column::Column, predicate::Predicate},
};
use anyhow::*;
use async_graphql_value::ConstValue;

use maybe_owned::MaybeOwned;
use payas_model::{model::predicate::*, sql::PhysicalTable};

pub trait PredicateParameterMapper<'a> {
    fn map_to_predicate(
        &'a self,
        argument_value: &'a ConstValue,
        query_context: &'a QueryContext<'a>,
    ) -> Result<(Predicate<'a>, Option<TableJoin>)>;
}

/// Table dependencies tree suitable for computing a join
#[derive(Debug)]
pub struct TableJoin<'a> {
    /// The base table being joined. In above example, "concerts"
    pub table: &'a PhysicalTable,
    /// The tables being joined. In above example, ("venue1_id", "venues") and ("venue2_id", "venues")
    pub dependencies: Vec<(JoinDependency, TableJoin<'a>)>,
}

impl<'a> PredicateParameterMapper<'a> for PredicateParameter {
    fn map_to_predicate(
        &'a self,
        argument_value: &'a ConstValue,
        query_context: &'a QueryContext<'a>,
    ) -> Result<(Predicate<'a>, Option<TableJoin>)> {
        let system = query_context.get_system();
        let parameter_type = &system.predicate_types[self.type_id];

        match &parameter_type.kind {
            PredicateParameterTypeKind::ImplicitEqual => {
                let (op_key_column, op_value_column) =
                    operands(self, argument_value, query_context);
                Ok((Predicate::Eq(op_key_column, op_value_column.into()), None))
            }
            PredicateParameterTypeKind::Operator(parameters) => {
                let predicate = parameters.iter().fold(Predicate::True, |acc, parameter| {
                    let arg = query_context.get_argument_field(argument_value, &parameter.name);
                    let new_predicate = match arg {
                        Some(op_value) => {
                            let (op_key_column, op_value_column) =
                                operands(self, op_value, query_context);
                            Predicate::from_name(
                                &parameter.name,
                                op_key_column,
                                op_value_column.into(),
                            )
                        }
                        None => Predicate::True,
                    };

                    Predicate::and(acc, new_predicate)
                });
                Ok((predicate, None))
            }
            PredicateParameterTypeKind::Composite {
                field_params,
                logical_op_params,
            } => {
                let underlying_type = &system.types[self.underlying_type_id];
                let underlying_table_id = underlying_type
                    .table_id()
                    .expect("Table could not be found");
                let underlying_table = &system.tables[underlying_table_id];

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
                                    let mut dependencies = vec![];

                                    for argument in arguments.iter() {
                                        let (mapped_predicate, mapped_dependency) =
                                            self.map_to_predicate(argument, query_context)?;
                                        new_predicate =
                                            predicate_connector(new_predicate, mapped_predicate);

                                        if let Some(mapped_dependency) = mapped_dependency {
                                            dependencies.push((
                                                self.join_dependency.clone().unwrap(),
                                                mapped_dependency,
                                            ));
                                        }
                                    }

                                    let table_join = TableJoin {
                                        table: underlying_table,
                                        dependencies,
                                    };

                                    Ok((new_predicate, Some(table_join)))
                                } else {
                                    bail!(
                                        "This logical operation predicate needs a list of queries"
                                    )
                                }
                            }

                            "not" => {
                                let (new_predicate, new_table_dependency) = self
                                    .map_to_predicate(logical_op_argument_value, query_context)?;

                                let table_join = TableJoin {
                                    table: underlying_table,
                                    dependencies: vec![(
                                        self.join_dependency.clone().unwrap(),
                                        new_table_dependency.unwrap(),
                                    )],
                                };
                                Ok((
                                    Predicate::Not(Box::new(new_predicate.into())),
                                    Some(table_join),
                                ))
                            }

                            _ => todo!(),
                        }
                    }

                    _ => {
                        // we are dealing with field predicate arguments
                        // map field argument values into their respective predicates
                        let mut new_predicate = Predicate::True;
                        let mut dependencies = vec![];

                        for parameter in field_params.iter() {
                            let arg =
                                query_context.get_argument_field(argument_value, &parameter.name);

                            let (field_predicate, field_table_dependency) = match arg {
                                Some(argument_value_component) => parameter
                                    .map_to_predicate(argument_value_component, query_context)?,
                                None => (Predicate::True, None),
                            };

                            new_predicate = Predicate::and(new_predicate, field_predicate);
                            if let Some(mapped_dependency) = field_table_dependency {
                                dependencies.push((
                                    parameter.join_dependency.clone().unwrap(),
                                    mapped_dependency,
                                ));
                            }
                        }

                        let table_join = TableJoin {
                            table: underlying_table,
                            dependencies,
                        };

                        Ok((new_predicate, Some(table_join)))
                    }
                }
            }
        }
    }
}

fn operands<'a>(
    param: &'a PredicateParameter,
    op_value: &'a ConstValue,
    query_context: &'a QueryContext<'a>,
) -> (MaybeOwned<'a, Column<'a>>, Column<'a>) {
    let system = query_context.get_system();

    let op_physical_column = &param
        .join_dependency
        .as_ref()
        .expect("Could not find join dependency")
        .self_column_id
        .get_column(system);
    let op_key_column = Column::Physical(op_physical_column).into();
    let op_value_column = query_context.literal_column(op_value, op_physical_column);
    (op_key_column, op_value_column.unwrap())
}
