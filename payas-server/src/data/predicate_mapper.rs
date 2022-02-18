use crate::execution::query_context::{cast_value, QueryContext};
use anyhow::*;
use async_graphql_value::ConstValue;

use payas_model::{
    model::{column_id::ColumnId, predicate::*, system::ModelSystem},
    sql::{column::PhysicalColumn, PhysicalTable},
};
use payas_sql::asql::{
    column_path::{ColumnPath, ColumnPathLink},
    predicate::AbstractPredicate,
};

pub trait PredicateParameterMapper<'a> {
    fn map_to_predicate(
        &'a self,
        argument_value: &'a ConstValue,
        parent_column_path: Option<ColumnIdPath>,
        query_context: &'a QueryContext<'a>,
    ) -> Result<AbstractPredicate<'a>>;
}

impl<'a> PredicateParameterMapper<'a> for PredicateParameter {
    fn map_to_predicate(
        &'a self,
        argument_value: &'a ConstValue,
        parent_column_path: Option<ColumnIdPath>,
        query_context: &'a QueryContext<'a>,
    ) -> Result<AbstractPredicate<'a>> {
        let system = query_context.get_system();
        let parameter_type = &system.predicate_types[self.type_id];

        match &parameter_type.kind {
            PredicateParameterTypeKind::ImplicitEqual => {
                let (op_key_path, op_value_path) =
                    operands(self, argument_value, parent_column_path, query_context)?;

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
                            let arg =
                                query_context.get_argument_field(argument_value, &parameter.name);
                            let new_predicate = match arg {
                                Some(op_value) => {
                                    let (op_key_column, op_value_column) = operands(
                                        self,
                                        op_value,
                                        parent_column_path.clone(),
                                        query_context,
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
                                        let arg_predicate = self.map_to_predicate(
                                            argument,
                                            parent_column_path.clone(),
                                            query_context,
                                        )?;
                                        new_predicate = predicate_connector(
                                            Box::new(new_predicate),
                                            Box::new(arg_predicate),
                                        );
                                    }

                                    Ok(new_predicate)
                                } else {
                                    bail!(
                                        "This logical operation predicate needs a list of queries"
                                    )
                                }
                            }

                            "not" => {
                                let arg_predicate = self.map_to_predicate(
                                    logical_op_argument_value,
                                    parent_column_path,
                                    query_context,
                                )?;

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
                            let arg =
                                query_context.get_argument_field(argument_value, &parameter.name);

                            let new_column_path =
                                to_column_id_path(&parent_column_path, &self.column_path_link);

                            let field_predicate = match arg {
                                Some(argument_value_component) => parameter.map_to_predicate(
                                    argument_value_component,
                                    new_column_path,
                                    query_context,
                                )?,
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
}

fn operands<'a>(
    param: &'a PredicateParameter,
    op_value: &'a ConstValue,
    parent_column_path: Option<ColumnIdPath>,
    query_context: &'a QueryContext<'a>,
) -> Result<(ColumnPath<'a>, ColumnPath<'a>)> {
    let system = query_context.get_system();

    let op_physical_column = &param
        .column_path_link
        .as_ref()
        .expect("Could not find column path link while forming operands")
        .self_column_id
        .get_column(system);
    let op_value = cast_value(op_value, &op_physical_column.typ);

    op_value.map(move |op_value| {
        (
            to_column_path(&parent_column_path, &param.column_path_link, system),
            ColumnPath::Literal(op_value.unwrap().into()),
        )
    })
}

fn to_column_path_link<'a>(link: &ColumnIdPathLink, system: &'a ModelSystem) -> ColumnPathLink<'a> {
    ColumnPathLink {
        self_column: to_column_table(link.self_column_id, system),
        linked_column: link
            .linked_column_id
            .map(|linked_column_id| to_column_table(linked_column_id, system)),
    }
}

fn to_column_table(column_id: ColumnId, system: &ModelSystem) -> (&PhysicalColumn, &PhysicalTable) {
    let column = column_id.get_column(system);
    let table = &system
        .tables
        .iter()
        .find(|(_, table)| table.name == column.table_name)
        .map(|(_, table)| table)
        .unwrap_or_else(|| panic!("Table {} not found", column.table_name));

    (column, table)
}

pub fn to_column_path<'a>(
    parent_column_id_path: &Option<ColumnIdPath>,
    next_column_id_path_link: &Option<ColumnIdPathLink>,
    system: &'a ModelSystem,
) -> ColumnPath<'a> {
    let mut path: Vec<_> = match parent_column_id_path {
        Some(parent_column_id_path) => parent_column_id_path
            .path
            .iter()
            .map(|link| to_column_path_link(link, system))
            .collect(),
        None => vec![],
    };

    if let Some(next_column_id_path_link) = next_column_id_path_link {
        path.push(to_column_path_link(next_column_id_path_link, system));
    }

    ColumnPath::Physical(path)
}

fn to_column_id_path(
    parent_column_id_path: &Option<ColumnIdPath>,
    next_column_id_path_link: &Option<ColumnIdPathLink>,
) -> Option<ColumnIdPath> {
    match (parent_column_id_path, next_column_id_path_link) {
        (Some(parent_column_id_path), Some(next_column_id_path_link)) => {
            let mut path: Vec<_> = parent_column_id_path.path.clone();
            path.push(next_column_id_path_link.clone());
            Some(ColumnIdPath { path })
        }
        (Some(parent_column_id_path), None) => Some(parent_column_id_path.clone()),
        (None, Some(next_column_id_path_link)) => Some(ColumnIdPath {
            path: vec![next_column_id_path_link.clone()],
        }),
        (None, None) => None,
    }
}
