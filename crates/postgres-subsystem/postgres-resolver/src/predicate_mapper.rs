// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt};

use core_plugin_interface::core_resolver::context::RequestContext;
use core_plugin_interface::core_resolver::value::Val;
use exo_sql::{
    AbstractPredicate, CaseSensitivity, ColumnPath, ParamEquality, PhysicalColumnPath, Predicate,
};
use futures::future::try_join_all;
use postgres_model::{
    predicate::{PredicateParameter, PredicateParameterTypeKind},
    subsystem::PostgresSubsystem,
};

use crate::{
    auth_util::check_retrieve_access,
    cast::literal_column_path,
    column_path_util::to_column_path,
    sql_mapper::{extract_and_map, SQLMapper},
    util::{get_argument_field, Arguments},
};

use super::postgres_execution_error::PostgresExecutionError;

#[derive(Debug)]
struct PredicateParamInput<'a> {
    pub param: &'a PredicateParameter,
    pub parent_column_path: Option<PhysicalColumnPath>,
}

#[async_trait]
impl<'a> SQLMapper<'a, AbstractPredicate> for PredicateParamInput<'a> {
    async fn to_sql(
        self,
        argument: &'a Val,
        subsystem: &'a PostgresSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractPredicate, PostgresExecutionError> {
        let parameter_type = &subsystem.predicate_types[self.param.typ.innermost().type_id];

        match &parameter_type.kind {
            PredicateParameterTypeKind::ImplicitEqual => {
                let (op_key_path, op_value_path) =
                    operands(self.param, argument, &self.parent_column_path, subsystem)?;

                Ok(AbstractPredicate::eq(op_key_path, op_value_path))
            }
            PredicateParameterTypeKind::Reference(parameters) => {
                parameters
                    .iter()
                    .try_fold(AbstractPredicate::True, |acc, parameter| {
                        let arg = get_argument_field(argument, &parameter.name);

                        match arg {
                            Some(arg) => {
                                let param_column_id = &self
                                    .param
                                    .column_path_link
                                    .as_ref()
                                    .unwrap()
                                    .self_column_id()
                                    .clone();

                                let param_column_path = ColumnPath::Physical(
                                    PhysicalColumnPath::leaf(*param_column_id),
                                );

                                let param_physical_column =
                                    param_column_id.get_column(&subsystem.database);

                                let op_value = literal_column_path(arg, param_physical_column)?;

                                let new_predicate =
                                    AbstractPredicate::eq(param_column_path, op_value);

                                Ok(AbstractPredicate::and(acc, new_predicate))
                            }
                            None => Err(PostgresExecutionError::Validation(
                                self.param.name.clone(),
                                format!("Reference parameter {} is missing", parameter.name),
                            ))?,
                        }
                    })
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
                                        &self.parent_column_path,
                                        subsystem,
                                    )
                                    .expect("Could not get operands");

                                    predicate_from_name(
                                        &parameter.name,
                                        op_key_column,
                                        op_value_column,
                                    )
                                }
                                None => AbstractPredicate::True,
                            };

                            AbstractPredicate::and(acc, new_predicate)
                        });

                Ok(predicate)
            }
            PredicateParameterTypeKind::Composite {
                field_params,
                logical_op_params,
            } => {
                // first, match any logical op predicates the argument_value might contain
                // logical_op_argument_value is of the form operation and value pair. For example,
                // `and: [{name: {eq: "foo"}}, {id: {lt: 1}}]` will be mapped to `("and", Some([{name: {eq: "foo"}}, {id: {lt: 1}}]))`
                let logical_op_argument_value: (&str, Option<&Val>) = logical_op_params
                    .iter()
                    .map(|parameter| {
                        (
                            parameter.name.as_str(),
                            get_argument_field(argument, &parameter.name),
                        )
                    })
                    .try_fold(("", None), |(acc_name, acc_result), (name, result)| {
                        if acc_result.is_some() && result.is_some() {
                            Err(PostgresExecutionError::Validation(
                                self.param.name.to_string(),
                                "Cannot specify more than one logical operation on the same level"
                                    .into(),
                            ))
                        } else if acc_result.is_some() && result.is_none() {
                            Ok((acc_name, acc_result))
                        } else {
                            Ok((name, result))
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
                                if let Val::List(arguments) = logical_op_argument_value {
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
                                        "and" => AbstractPredicate::and,
                                        "or" => AbstractPredicate::or,
                                        _ => todo!(),
                                    };

                                    let predicates = arguments.iter().map(|argument| {
                                        PredicateParamInput {
                                            param: self.param,
                                            parent_column_path: self.parent_column_path.clone(),
                                        }
                                        .to_sql(
                                            argument,
                                            subsystem,
                                            request_context,
                                        )
                                    });

                                    let predicates: Result<Vec<_>, _> =
                                        try_join_all(predicates).await;

                                    Ok(predicates?
                                        .into_iter()
                                        .fold(identity_predicate, |acc, predicate| {
                                            predicate_connector(acc, predicate)
                                        }))
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
                                .to_sql(logical_op_argument_value, subsystem, request_context)
                                .await?;

                                Ok(!arg_predicate)
                            }

                            _ => todo!(),
                        }
                    }

                    _ => {
                        // we are dealing with field predicate arguments
                        // map field argument values into their respective predicates

                        let provided_field_params = field_params.iter().flat_map(|parameter| {
                            let arg = get_argument_field(argument, &parameter.name);
                            arg.map(|arg| (arg, parameter))
                        });

                        futures::stream::iter(provided_field_params)
                            .map(Ok)
                            .try_fold(AbstractPredicate::True, |acc, (arg, parameter)| async {
                                let new_column_path = to_column_path(
                                    &self.parent_column_path,
                                    &self.param.column_path_link,
                                );

                                let field_access = match parameter.access {
                                    Some(ref access) => {
                                        check_retrieve_access(
                                            &subsystem.database_access_expressions[access.read],
                                            subsystem,
                                            request_context,
                                        )
                                        .await?
                                    }
                                    None => AbstractPredicate::True,
                                };

                                if field_access != AbstractPredicate::True {
                                    Err(PostgresExecutionError::Authorization)
                                } else {
                                    let param_predicate = PredicateParamInput {
                                        param: parameter,
                                        parent_column_path: new_column_path,
                                    }
                                    .to_sql(arg, subsystem, request_context)
                                    .await?;

                                    Ok(AbstractPredicate::and(acc, param_predicate))
                                }
                            })
                            .await
                    }
                }
            }
            PredicateParameterTypeKind::Vector => {
                let value = argument.get("value").unwrap();

                let value_vec: Vec<f32> = match value {
                    Val::String(s) => serde_json::from_str(s)
                        .map_err(|e| PostgresExecutionError::Generic(e.to_string())),
                    _ => Err(PostgresExecutionError::Validation(
                        "value".into(),
                        "Invalid vector order by parameter".into(),
                    )),
                }?;
                let distance = argument.get("distance").unwrap();

                // subsystem.predicate_types.get(i)

                // operands(param, distance, parent_column_path, subsystem);

                todo!(
                    "Vector type not supported yet. Argument: {:?} {:?}",
                    value_vec,
                    distance
                )
            }
        }
    }

    fn param_name(&self) -> &str {
        &self.param.name
    }
}

/// Map predicate from GraphQL operation name to a Predicate
pub fn predicate_from_name<C: PartialEq + ParamEquality>(
    op_name: &str,
    lhs: C,
    rhs: C,
) -> Predicate<C> {
    match op_name {
        "eq" => Predicate::Eq(lhs, rhs),
        "neq" => Predicate::Neq(lhs, rhs),
        "lt" => Predicate::Lt(lhs, rhs),
        "lte" => Predicate::Lte(lhs, rhs),
        "gt" => Predicate::Gt(lhs, rhs),
        "gte" => Predicate::Gte(lhs, rhs),
        "like" => Predicate::StringLike(lhs, rhs, CaseSensitivity::Sensitive),
        "ilike" => Predicate::StringLike(lhs, rhs, CaseSensitivity::Insensitive),
        "startsWith" => Predicate::StringStartsWith(lhs, rhs),
        "endsWith" => Predicate::StringEndsWith(lhs, rhs),
        "contains" => Predicate::JsonContains(lhs, rhs),
        "containedBy" => Predicate::JsonContainedBy(lhs, rhs),
        "matchKey" => Predicate::JsonMatchKey(lhs, rhs),
        "matchAnyKey" => Predicate::JsonMatchAnyKey(lhs, rhs),
        "matchAllKeys" => Predicate::JsonMatchAllKeys(lhs, rhs),
        _ => todo!(),
    }
}

fn operands<'a>(
    param: &'a PredicateParameter,
    op_value: &'a Val,
    parent_column_path: &Option<PhysicalColumnPath>,
    subsystem: &'a PostgresSubsystem,
) -> Result<(ColumnPath, ColumnPath), PostgresExecutionError> {
    let op_physical_column_id = param
        .column_path_link
        .as_ref()
        .expect("Could not find column path link while forming operands")
        .self_column_id();
    let op_physical_column = op_physical_column_id.get_column(&subsystem.database);

    let op_value = literal_column_path(op_value, op_physical_column)?;

    Ok((
        ColumnPath::Physical(to_column_path(parent_column_path, &param.column_path_link).unwrap()),
        op_value,
    ))
}

pub async fn compute_predicate<'a>(
    param: &'a PredicateParameter,
    arguments: &'a Arguments,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    extract_and_map(
        PredicateParamInput {
            param,
            parent_column_path: None,
        },
        arguments,
        subsystem,
        request_context,
    )
    .await
    .map(|predicate| predicate.unwrap_or(AbstractPredicate::True))
}
