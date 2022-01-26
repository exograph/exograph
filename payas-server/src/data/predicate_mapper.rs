use std::collections::{hash_map::Entry, HashMap};

use crate::{
    execution::query_context::QueryContext,
    sql::{column::Column, predicate::Predicate},
};
use anyhow::*;
use async_graphql_value::ConstValue;

use maybe_owned::MaybeOwned;
use payas_model::{
    model::{predicate::*, system::ModelSystem},
    sql::PhysicalTable,
};

pub trait PredicateParameterMapper<'a> {
    fn map_to_predicate(
        &'a self,
        argument_value: &'a ConstValue,
        query_context: &'a QueryContext<'a>,
    ) -> Result<(Predicate<'a>, Vec<ColumnPath>)>;
}

#[derive(Debug)]
pub struct TableJoin<'a> {
    /// The base table being joined. In above example, "concerts"
    pub table: &'a PhysicalTable,
    /// The tables being joined. In above example, ("venue1_id", "venues") and ("venue2_id", "venues")
    pub dependencies: Vec<(ColumnPathLink, TableJoin<'a>)>,
}

impl<'a> TableJoin<'a> {
    /// Compute TableJoin from a list of column paths
    /// If the following path is given:
    /// ```no_rust
    /// [
    ///     (concert.id, concert_artists.concert_id) -> (concert_artists.artist_id, artists.id) -> (artists.name, None)
    ///     (concert.id, concert_artists.concert_id) -> (concert_artists.artist_id, artists.id) -> (artists.address_id, address.id) -> (address.city, None)
    ///     (concert.venue_id, venue.id) -> (venue.name, None)
    /// ]
    /// ```
    /// then the result will be the join needed to access the leaf columns:
    /// ```no_rust
    /// TableJoin {
    ///    table: concerts,
    ///    dependencies: [
    ///       ((concert.id, concert_artists.concert_id), TableJoin {
    ///          table: concert_artists,
    ///          dependencies: [
    ///             ((concert_artists.artist_id, artists.id), TableJoin {
    ///                table: artists,
    ///                dependencies: [
    ///                   ((artists.address_id, address.id), TableJoin {
    ///                      table: address,
    ///                      dependencies: []
    ///                   }),
    ///                ]
    ///             }),
    ///       ((concert.venue_id, venue.id), TableJoin {
    ///            table: venue,
    ///            dependencies: []
    ///       }),
    ///    ]
    /// }
    /// ```
    pub fn from_column_path(paths_list: Vec<ColumnPath>, system: &'a ModelSystem) -> Option<Self> {
        let tables = &system.tables;

        paths_list
            .first()
            .and_then(|path| path.path.first().map(|dep| dep.self_column_id.table_id))
            .map(|table_id| {
                let table = &tables[table_id];

                let mut grouped: HashMap<ColumnPathLink, Vec<Vec<ColumnPathLink>>> = HashMap::new();
                for paths in paths_list {
                    match &paths.path[..] {
                        [head, tail @ ..] => {
                            if head.linked_column_id.is_some() {
                                let existing = grouped.entry(head.clone());

                                match existing {
                                    Entry::Occupied(mut entry) => {
                                        entry.get_mut().push(tail.to_vec())
                                    }
                                    Entry::Vacant(entry) => {
                                        entry.insert(vec![tail.to_vec()]);
                                    }
                                }
                            }
                        }
                        _ => {
                            panic!("Invalid paths list")
                        }
                    }
                }

                Self {
                    table,
                    dependencies: grouped
                        .into_iter()
                        .map(|(head, tail)| {
                            let inner_join = Self::from_column_path(
                                tail.into_iter().map(|path| ColumnPath { path }).collect(),
                                system,
                            )
                            .unwrap();

                            (head, inner_join)
                        })
                        .collect(),
                }
            })
    }
}

impl<'a> PredicateParameterMapper<'a> for PredicateParameter {
    fn map_to_predicate(
        &'a self,
        argument_value: &'a ConstValue,
        query_context: &'a QueryContext<'a>,
    ) -> Result<(Predicate<'a>, Vec<ColumnPath>)> {
        let system = query_context.get_system();
        let parameter_type = &system.predicate_types[self.type_id];

        match &parameter_type.kind {
            PredicateParameterTypeKind::ImplicitEqual => {
                let (op_key_column, op_value_column) =
                    operands(self, argument_value, query_context);
                Ok((Predicate::Eq(op_key_column, op_value_column.into()), vec![]))
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

                let column_path = match &self.column_path_link {
                    Some(link) => ColumnPath {
                        path: vec![link.clone()],
                    },
                    None => ColumnPath { path: vec![] },
                };
                Ok((predicate, vec![column_path]))
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
                                    let mut column_paths: Vec<ColumnPath> = vec![];

                                    for argument in arguments.iter() {
                                        let (arg_predicate, arg_column_paths) =
                                            self.map_to_predicate(argument, query_context)?;
                                        new_predicate =
                                            predicate_connector(new_predicate, arg_predicate);
                                        column_paths = column_paths
                                            .into_iter()
                                            .chain(arg_column_paths.into_iter())
                                            .collect();
                                    }

                                    Ok((new_predicate, column_paths))
                                } else {
                                    bail!(
                                        "This logical operation predicate needs a list of queries"
                                    )
                                }
                            }

                            "not" => {
                                let (arg_predicate, arg_column_paths) = self
                                    .map_to_predicate(logical_op_argument_value, query_context)?;

                                Ok((
                                    Predicate::Not(Box::new(arg_predicate.into())),
                                    arg_column_paths,
                                ))
                            }

                            _ => todo!(),
                        }
                    }

                    _ => {
                        // we are dealing with field predicate arguments
                        // map field argument values into their respective predicates
                        let mut new_predicate = Predicate::True;
                        let mut column_paths = vec![];

                        for parameter in field_params.iter() {
                            let arg =
                                query_context.get_argument_field(argument_value, &parameter.name);

                            let (field_predicate, field_column_path) = match arg {
                                Some(argument_value_component) => parameter
                                    .map_to_predicate(argument_value_component, query_context)?,
                                None => (Predicate::True, vec![]),
                            };

                            if let Some(column_path_link) = self.column_path_link.clone() {
                                for mut column_path in field_column_path.into_iter() {
                                    column_path.path.insert(0, column_path_link.clone());
                                    column_paths.push(column_path);
                                }
                            } else {
                                column_paths = column_paths
                                    .into_iter()
                                    .chain(field_column_path.into_iter())
                                    .collect();
                            }

                            new_predicate = Predicate::and(new_predicate, field_predicate);
                        }

                        Ok((new_predicate, column_paths))
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
        .column_path_link
        .as_ref()
        .expect("Could not find column path link while forming operands")
        .self_column_id
        .get_column(system);
    let op_key_column = Column::Physical(op_physical_column).into();
    let op_value_column = query_context.literal_column(op_value, op_physical_column);
    (op_key_column, op_value_column.unwrap())
}
