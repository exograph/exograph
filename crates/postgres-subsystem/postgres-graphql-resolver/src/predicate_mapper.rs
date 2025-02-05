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

use common::context::RequestContext;
use common::value::Val;
use exo_sql::{
    AbstractPredicate, CaseSensitivity, ColumnPath, ColumnPathLink, ParamEquality,
    PhysicalColumnPath, PhysicalColumnType, Predicate,
};

use exo_sql::{NumericComparator, SQLParamContainer};

use futures::future::try_join_all;
use postgres_graphql_model::{
    predicate::{PredicateParameter, PredicateParameterTypeKind},
    subsystem::PostgresGraphQLSubsystem,
};

use crate::{
    auth_util::check_retrieve_access,
    column_path_util::to_column_path,
    sql_mapper::{extract_and_map, SQLMapper},
    util::{get_argument_field, Arguments},
};

use postgres_core_resolver::cast::{cast_value, literal_column_path};

use crate::util::to_pg_vector;

use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;

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
        subsystem: &'a PostgresGraphQLSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractPredicate, PostgresExecutionError> {
        let parameter_type = &subsystem.predicate_types[self.param.typ.innermost().type_id];

        match &parameter_type.kind {
            PredicateParameterTypeKind::ImplicitEqual => {
                let (op_key_path, op_value_path) = operands(
                    self.param,
                    argument,
                    None,
                    &self.parent_column_path,
                    subsystem,
                )?;

                Ok(AbstractPredicate::eq(op_key_path, op_value_path))
            }
            PredicateParameterTypeKind::Reference(parameters) => {
                parameters
                    .iter()
                    .try_fold(AbstractPredicate::True, |acc, parameter| {
                        let arg = get_argument_field(argument, &parameter.name);

                        match arg {
                            Some(arg) => {
                                // For the reference parameter with composite primary key, we need to find
                                // the column id for the parameter. Then we can coerece the arguments to its type.

                                // Leaf column id is the column corresponding to the leaf parameter.
                                // For example, if the argument is `user: {email: "user1@example.com", orgId: 1}`,
                                // and the parameter is `email`, then the leaf column id is the column id for the `email` column.
                                let leaf_column_id = match parameter.column_path_link.as_ref() {
                                    Some(column_path_link) => {
                                        match &column_path_link.self_column_ids()[..] {
                                            [column_id] => *column_id,
                                            _ => panic!("Expected a single column id"),
                                        }
                                    }
                                    None => panic!("Expected column path link"),
                                };

                                let op_value = literal_column_path(
                                    arg,
                                    &leaf_column_id
                                        .get_column(&subsystem.core_subsystem.database)
                                        .typ,
                                    false,
                                )?;

                                let param_column_id = match &self.param.column_path_link {
                                    Some(ColumnPathLink::Leaf(column_id)) => *column_id,
                                    Some(ColumnPathLink::Relation(column_path_link)) => {
                                        // Composite pks. Find the index of the column id in the column pairs
                                        column_path_link
                                            .column_pairs
                                            .iter()
                                            .find_map(|column_pair| {
                                                if column_pair.foreign_column_id == leaf_column_id {
                                                    Some(column_pair.self_column_id)
                                                } else {
                                                    None
                                                }
                                            })
                                            .expect("Expected a matching column path link")
                                    }
                                    None => panic!("Expected column path link"),
                                };

                                let param_column_path =
                                    ColumnPath::Physical(PhysicalColumnPath::leaf(param_column_id));

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
                parameters
                    .iter()
                    .try_fold(AbstractPredicate::True, |acc, parameter| {
                        let arg = get_argument_field(argument, &parameter.name);
                        let new_predicate = match arg {
                            Some(op_value) => {
                                let arg_parameter_type =
                                    &subsystem.predicate_types[parameter.typ.innermost().type_id];

                                if matches!(
                                    arg_parameter_type.kind,
                                    PredicateParameterTypeKind::Vector
                                ) {
                                    let value = op_value.get("distanceTo").unwrap();
                                    let vector_value = to_pg_vector(value, &parameter.name)?;

                                    let distance = op_value.get("distance").unwrap();
                                    match distance {
                                        Val::Object(map) => {
                                            assert!(map.len() == 1);
                                            let operator = map.keys().next().unwrap();
                                            let threshold = map.values().next().unwrap();

                                            let distance_comparator = match operator.as_str() {
                                                "eq" => Ok(NumericComparator::Eq),
                                                "neq" => Ok(NumericComparator::Neq),
                                                "lt" => Ok(NumericComparator::Lt),
                                                "lte" => Ok(NumericComparator::Lte),
                                                "gt" => Ok(NumericComparator::Gt),
                                                "gte" => Ok(NumericComparator::Gte),
                                                _ => Err(PostgresExecutionError::Validation(
                                                    "distance".into(),
                                                    "Invalid distance operator".into(),
                                                )),
                                            }?;

                                            let threshold = cast_value(
                                                threshold,
                                                &exo_sql::PhysicalColumnType::Float {
                                                    bits: exo_sql::FloatBits::_53,
                                                },
                                                false,
                                            )?
                                            .unwrap();
                                            let target_vector =
                                                SQLParamContainer::f32_array(vector_value);

                                            Ok(AbstractPredicate::VectorDistance(
                                                ColumnPath::Physical(
                                                    to_column_path(
                                                        &self.parent_column_path,
                                                        &self.param.column_path_link,
                                                    )
                                                    .unwrap(),
                                                ),
                                                ColumnPath::Param(target_vector),
                                                self.param
                                                    .vector_distance_function
                                                    .unwrap_or_default(),
                                                distance_comparator,
                                                ColumnPath::Param(threshold),
                                            ))
                                        }
                                        _ => Err(PostgresExecutionError::Validation(
                                            "distance".into(),
                                            "Invalid distance parameter".into(),
                                        )),
                                    }
                                } else {
                                    let override_op_value_type = match parameter.name.as_str() {
                                        "matchAllKeys" | "matchAnyKey" => {
                                            Some(PhysicalColumnType::Array {
                                                typ: Box::new(PhysicalColumnType::String {
                                                    max_length: None,
                                                }),
                                            })
                                        }
                                        _ => None,
                                    };

                                    let (op_key_column, op_value_column) = operands(
                                        self.param,
                                        op_value,
                                        override_op_value_type.as_ref(),
                                        &self.parent_column_path,
                                        subsystem,
                                    )
                                    .expect("Could not get operands");

                                    Ok(predicate_from_name(
                                        &parameter.name,
                                        op_key_column,
                                        op_value_column,
                                    ))
                                }
                            }
                            None => Ok(AbstractPredicate::True),
                        };

                        new_predicate
                            .map(|new_predicate| AbstractPredicate::and(acc, new_predicate))
                    })
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
                                            &subsystem.core_subsystem.database_access_expressions
                                                [access.read],
                                            subsystem,
                                            request_context,
                                        )
                                        .await?
                                    }
                                    None => AbstractPredicate::True,
                                };

                                if field_access == Predicate::False {
                                    Err(PostgresExecutionError::Authorization)
                                } else {
                                    let param_predicate = PredicateParamInput {
                                        param: parameter,
                                        parent_column_path: new_column_path,
                                    }
                                    .to_sql(arg, subsystem, request_context)
                                    .await?;

                                    Ok(AbstractPredicate::and(
                                        field_access,
                                        AbstractPredicate::and(acc, param_predicate),
                                    ))
                                }
                            })
                            .await
                    }
                }
            }
            PredicateParameterTypeKind::Vector => Err(PostgresExecutionError::Validation(
                self.param.name.clone(),
                "Vector argument not expected in this context".into(),
            )),
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
    op_value_type: Option<&PhysicalColumnType>,
    parent_column_path: &Option<PhysicalColumnPath>,
    subsystem: &'a PostgresGraphQLSubsystem,
) -> Result<(ColumnPath, ColumnPath), PostgresExecutionError> {
    let op_param_column_path = param
        .column_path_link
        .as_ref()
        .expect("Could not find column path link while forming operands");
    let op_physical_column_ids = op_param_column_path.self_column_ids();
    assert!(
        op_physical_column_ids.len() == 1,
        "Operand must be non-composite columns"
    );
    let op_physical_column_id = op_physical_column_ids[0];
    let op_physical_column = op_physical_column_id.get_column(&subsystem.core_subsystem.database);

    let op_value = literal_column_path(
        op_value,
        op_value_type.unwrap_or(&op_physical_column.typ),
        false,
    )?;

    Ok((
        ColumnPath::Physical(to_column_path(parent_column_path, &param.column_path_link).unwrap()),
        op_value,
    ))
}

pub async fn compute_predicate<'a>(
    params: &'a [&'a PredicateParameter],
    arguments: &'a Arguments,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    let predicates = futures::future::try_join_all(params.iter().map(|param| async {
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
    }))
    .await?;

    let predicates = predicates.into_iter().flatten();

    Ok(predicates.fold(AbstractPredicate::True, |acc, predicate| {
        AbstractPredicate::and(acc, predicate)
    }))
}
