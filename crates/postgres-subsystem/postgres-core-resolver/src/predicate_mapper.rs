// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Core predicate mapping logic shared between GraphQL and RPC resolvers.

use async_trait::async_trait;
use common::context::RequestContext;
use common::value::Val;
use core_model::mapped_arena::SerializableSlab;
use core_resolver::access_solver::AccessSolver;
use exo_sql::{
    AbstractPredicate, ArrayColumnType, ColumnPath, ColumnPathLink, Database, NumericComparator,
    PhysicalColumnPath, PhysicalColumnType, PhysicalColumnTypeExt, Predicate, SQLParamContainer,
    StringColumnType,
};
use futures::future::{BoxFuture, FutureExt, try_join_all};
use futures::{StreamExt, TryStreamExt};
use postgres_core_model::predicate::{
    PredicateParameter, PredicateParameterType, PredicateParameterTypeKind,
};
use postgres_core_model::subsystem::PostgresCoreSubsystem;

use crate::cast::{cast_value, literal_column_path};
use crate::column_path_util::to_column_path;
use crate::postgres_execution_error::PostgresExecutionError;
use crate::predicate_util::{get_argument_field, predicate_from_name, to_pg_vector};

/// Trait for checking field-level access during predicate mapping.
#[async_trait]
trait FieldAccessChecker: Send + Sync {
    /// Check if the current request has access to use this predicate parameter in a query.
    ///
    /// Returns:
    /// - `Ok(AbstractPredicate::True)` if access is allowed unconditionally
    /// - `Ok(predicate)` if access is allowed with a restricting predicate
    /// - `Err(PostgresExecutionError::Authorization)` if access is denied
    async fn check_field_access(
        &self,
        param: &PredicateParameter,
        request_context: &RequestContext<'_>,
    ) -> Result<AbstractPredicate, PostgresExecutionError>;
}

/// Field access checker that uses the core subsystem's access expressions.
struct CoreFieldAccessChecker<'a> {
    subsystem: &'a PostgresCoreSubsystem,
}

#[async_trait]
impl FieldAccessChecker for CoreFieldAccessChecker<'_> {
    async fn check_field_access(
        &self,
        param: &PredicateParameter,
        request_context: &RequestContext<'_>,
    ) -> Result<AbstractPredicate, PostgresExecutionError> {
        match param.access {
            Some(ref access) => {
                let expr = &self.subsystem.database_access_expressions[access.read];
                Ok(self
                    .subsystem
                    .solve(request_context, None, expr)
                    .await?
                    .map(|p| p.0)
                    .resolve())
            }
            None => Ok(AbstractPredicate::True),
        }
    }
}

/// Core predicate mapping function.
fn map_predicate<'a, F: FieldAccessChecker>(
    param: &'a PredicateParameter,
    argument: &'a Val,
    parent_column_path: Option<PhysicalColumnPath>,
    database: &'a Database,
    predicate_types: &'a SerializableSlab<PredicateParameterType>,
    request_context: &'a RequestContext<'a>,
    access_checker: &'a F,
) -> BoxFuture<'a, Result<AbstractPredicate, PostgresExecutionError>> {
    async move {
        let parameter_type = &predicate_types[param.typ.innermost().type_id];

        match &parameter_type.kind {
            PredicateParameterTypeKind::ImplicitEqual => {
                let (op_key_path, op_value_path) =
                    operands(param, argument, None, &parent_column_path, database)?;

                Ok(AbstractPredicate::eq(op_key_path, op_value_path))
            }
            PredicateParameterTypeKind::Reference(parameters) => {
                parameters
                    .iter()
                    .try_fold(AbstractPredicate::True, |acc, parameter| {
                        let arg = get_argument_field(argument, &parameter.name);

                        match arg {
                            Some(arg) => {
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
                                    leaf_column_id.get_column(database).typ.inner(),
                                    false,
                                )?;

                                let param_column_id = match &param.column_path_link {
                                    Some(ColumnPathLink::Leaf(column_id)) => *column_id,
                                    Some(ColumnPathLink::Relation(column_path_link)) => {
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
                                param.name.clone(),
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
                                    &predicate_types[parameter.typ.innermost().type_id];

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

                                            let float_type = exo_sql::FloatColumnType {
                                                bits: exo_sql::FloatBits::_53,
                                            };
                                            let threshold =
                                                cast_value(threshold, &float_type, false)?.unwrap();
                                            let target_vector =
                                                SQLParamContainer::f32_array(vector_value);

                                            Ok(AbstractPredicate::VectorDistance(
                                                ColumnPath::Physical(
                                                    to_column_path(
                                                        &parent_column_path,
                                                        &param.column_path_link,
                                                    )
                                                    .unwrap(),
                                                ),
                                                ColumnPath::Param(target_vector),
                                                param.vector_distance_function.unwrap_or_default(),
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
                                    let string_type = StringColumnType { max_length: None };
                                    let array_type = ArrayColumnType {
                                        typ: Box::new(string_type),
                                    };

                                    let override_op_value_type: Option<&dyn PhysicalColumnType> =
                                        match parameter.name.as_str() {
                                            "matchAllKeys" | "matchAnyKey" => Some(&array_type),
                                            _ => None,
                                        };

                                    let (op_key_column, op_value_column) = operands(
                                        param,
                                        op_value,
                                        override_op_value_type,
                                        &parent_column_path,
                                        database,
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
                                param.name.to_string(),
                                "Cannot specify more than one logical operation on the same level"
                                    .into(),
                            ))
                        } else if acc_result.is_some() && result.is_none() {
                            Ok((acc_name, acc_result))
                        } else {
                            Ok((name, result))
                        }
                    })?;

                match logical_op_argument_value {
                    (logical_op_name, Some(logical_op_argument_value)) => match logical_op_name {
                        "and" | "or" => {
                            if let Val::List(arguments) = logical_op_argument_value {
                                if arguments.is_empty() {
                                    return Err(PostgresExecutionError::Validation(
                                        param.name.clone(),
                                        "Logical operation predicate does not have any arguments"
                                            .into(),
                                    ));
                                }

                                let identity_predicate = match logical_op_name {
                                    "and" => AbstractPredicate::True,
                                    "or" => AbstractPredicate::False,
                                    _ => unreachable!(),
                                };

                                let predicate_connector = match logical_op_name {
                                    "and" => AbstractPredicate::and,
                                    "or" => AbstractPredicate::or,
                                    _ => unreachable!(),
                                };

                                let predicates = arguments.iter().map(|argument| {
                                    map_predicate(
                                        param,
                                        argument,
                                        parent_column_path.clone(),
                                        database,
                                        predicate_types,
                                        request_context,
                                        access_checker,
                                    )
                                });

                                let predicates: Result<Vec<_>, _> = try_join_all(predicates).await;

                                Ok(predicates?
                                    .into_iter()
                                    .fold(identity_predicate, |acc, predicate| {
                                        predicate_connector(acc, predicate)
                                    }))
                            } else {
                                Err(PostgresExecutionError::Validation(
                                    param.name.clone(),
                                    "This logical operation predicate needs a list of queries"
                                        .into(),
                                ))
                            }
                        }

                        "not" => {
                            let arg_predicate = map_predicate(
                                param,
                                logical_op_argument_value,
                                parent_column_path,
                                database,
                                predicate_types,
                                request_context,
                                access_checker,
                            )
                            .await?;

                            Ok(!arg_predicate)
                        }

                        _ => unreachable!(),
                    },

                    _ => {
                        // We are dealing with field predicate arguments
                        // Map field argument values into their respective predicates
                        let provided_field_params = field_params.iter().flat_map(|parameter| {
                            let arg = get_argument_field(argument, &parameter.name);
                            arg.map(|arg| (arg, parameter))
                        });

                        futures::stream::iter(provided_field_params)
                            .map(Ok)
                            .try_fold(AbstractPredicate::True, |acc, (arg, parameter)| async {
                                let new_column_path =
                                    to_column_path(&parent_column_path, &param.column_path_link);

                                // Check field-level access
                                let field_access = access_checker
                                    .check_field_access(parameter, request_context)
                                    .await?;

                                if field_access == Predicate::False {
                                    Err(PostgresExecutionError::Authorization)
                                } else {
                                    let param_predicate = map_predicate(
                                        parameter,
                                        arg,
                                        new_column_path,
                                        database,
                                        predicate_types,
                                        request_context,
                                        access_checker,
                                    )
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
                param.name.clone(),
                "Vector argument not expected in this context".into(),
            )),
        }
    }
    .boxed()
}

/// Compute operands for a predicate comparison.
fn operands<'a>(
    param: &'a PredicateParameter,
    op_value: &'a Val,
    op_value_type: Option<&dyn PhysicalColumnType>,
    parent_column_path: &Option<PhysicalColumnPath>,
    database: &'a Database,
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
    let op_physical_column = op_physical_column_id.get_column(database);

    let op_value = literal_column_path(
        op_value,
        op_value_type.unwrap_or(op_physical_column.typ.inner()),
        false,
    )?;

    Ok((
        ColumnPath::Physical(to_column_path(parent_column_path, &param.column_path_link).unwrap()),
        op_value,
    ))
}

/// Entry point for computing predicates with field-level access checking.
///
/// This is the primary API for both GraphQL and RPC resolvers. It uses the
/// subsystem's access expressions to enforce field-level access control.
pub async fn compute_predicate<'a>(
    param: &'a PredicateParameter,
    argument: &'a Val,
    subsystem: &'a PostgresCoreSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    let access_checker = CoreFieldAccessChecker { subsystem };

    map_predicate(
        param,
        argument,
        None,
        &subsystem.database,
        &subsystem.predicate_types,
        request_context,
        &access_checker,
    )
    .await
}
