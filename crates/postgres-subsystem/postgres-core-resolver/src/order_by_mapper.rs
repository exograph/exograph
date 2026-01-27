// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Core order-by mapping logic shared between GraphQL and RPC resolvers.

use async_trait::async_trait;
use common::context::RequestContext;
use common::value::Val;
use core_model::mapped_arena::SerializableSlab;
use core_resolver::access_solver::AccessSolver;
use exo_sql::{
    AbstractOrderBy, AbstractOrderByExpr, AbstractPredicate, ColumnPath, Ordering,
    PhysicalColumnPath, SQLParamContainer, VectorDistanceFunction,
};
use futures::future::join_all;
use postgres_core_model::order::{
    OrderByParameter, OrderByParameterType, OrderByParameterTypeKind,
};
use postgres_core_model::subsystem::PostgresCoreSubsystem;

use crate::column_path_util::to_column_path;
use crate::postgres_execution_error::PostgresExecutionError;
use crate::predicate_util::to_pg_vector;

/// Trait for checking field-level access during order-by mapping.
#[async_trait]
trait OrderByFieldAccessChecker: Send + Sync {
    /// Check if the current request has access to use this order-by parameter in a query.
    ///
    /// Returns:
    /// - `Ok(AbstractPredicate::True)` if access is allowed unconditionally
    /// - `Ok(predicate)` if access is allowed with a restricting predicate
    /// - `Err(PostgresExecutionError::Authorization)` if access is denied
    async fn check_field_access(
        &self,
        param: &OrderByParameter,
        request_context: &RequestContext<'_>,
    ) -> Result<AbstractPredicate, PostgresExecutionError>;
}

/// Field access checker that uses the core subsystem's access expressions.
struct CoreOrderByFieldAccessChecker<'a> {
    subsystem: &'a PostgresCoreSubsystem,
}

#[async_trait]
impl OrderByFieldAccessChecker for CoreOrderByFieldAccessChecker<'_> {
    async fn check_field_access(
        &self,
        param: &OrderByParameter,
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

/// Core order-by mapping function.
async fn map_order_by<'a, F: OrderByFieldAccessChecker>(
    param: &'a OrderByParameter,
    argument: &'a Val,
    parent_column_path: Option<PhysicalColumnPath>,
    order_by_types: &'a SerializableSlab<OrderByParameterType>,
    request_context: &'a RequestContext<'a>,
    access_checker: &'a F,
) -> Result<AbstractOrderBy, PostgresExecutionError> {
    let parameter_type = &order_by_types[param.typ.innermost().type_id];

    fn flatten<E>(order_bys: Result<Vec<AbstractOrderBy>, E>) -> Result<AbstractOrderBy, E> {
        let mapped = order_bys?.into_iter().flat_map(|elem| elem.0).collect();
        Ok(AbstractOrderBy(mapped))
    }

    match argument {
        Val::Object(elems) => {
            let mapped = elems.iter().map(|elem| {
                order_by_pair(
                    parameter_type,
                    elem.0,
                    elem.1,
                    parent_column_path.clone(),
                    order_by_types,
                    request_context,
                    access_checker,
                )
            });

            let mapped = join_all(mapped).await.into_iter().collect();

            flatten(mapped)
        }
        Val::List(elems) => {
            let mapped = elems.iter().map(|elem| {
                map_order_by(
                    param,
                    elem,
                    parent_column_path.clone(),
                    order_by_types,
                    request_context,
                    access_checker,
                )
            });

            let mapped = join_all(mapped).await.into_iter().collect();
            flatten(mapped)
        }

        _ => Err(PostgresExecutionError::Validation(
            param.name.clone(),
            format!("Invalid argument ('{argument}')"),
        )),
    }
}

async fn order_by_pair<'a, F: OrderByFieldAccessChecker>(
    typ: &'a OrderByParameterType,
    parameter_name: &str,
    parameter_value: &'a Val,
    parent_column_path: Option<PhysicalColumnPath>,
    order_by_types: &'a SerializableSlab<OrderByParameterType>,
    request_context: &'a RequestContext<'a>,
    access_checker: &'a F,
) -> Result<AbstractOrderBy, PostgresExecutionError> {
    match &typ.kind {
        OrderByParameterTypeKind::Composite { parameters } => {
            match parameters.iter().find(|p| p.name == parameter_name) {
                Some(parameter) => {
                    let field_access = access_checker
                        .check_field_access(parameter, request_context)
                        .await?;

                    if field_access != AbstractPredicate::True {
                        return Err(PostgresExecutionError::Authorization);
                    }

                    let base_param_type = &order_by_types[parameter.typ.innermost().type_id];

                    // If this is a leaf node ({something: ASC} kind), then resolve the ordering. If not, then recurse with a new parent column path.
                    let new_column_path =
                        to_column_path(&parent_column_path, &parameter.column_path_link);

                    match &base_param_type.kind {
                        OrderByParameterTypeKind::Primitive => {
                            let new_column_path = new_column_path.unwrap();
                            ordering(parameter_value).map(|ordering| {
                                AbstractOrderBy(vec![(
                                    AbstractOrderByExpr::Column(new_column_path),
                                    ordering,
                                )])
                            })
                        }
                        OrderByParameterTypeKind::Vector => match parameter_value {
                            Val::Object(elems) => {
                                let new_column_path = new_column_path.unwrap();

                                // These unwraps are safe, since the validation of the parameter type guarantees that these keys exist.
                                let value = elems.get("distanceTo").unwrap();

                                let default_order = Val::String("ASC".to_owned());
                                let order = elems.get("order").unwrap_or(&default_order);

                                let vector_value = to_pg_vector(value, parameter_name)?;

                                ordering(order).map(|ordering| {
                                    AbstractOrderBy(vec![(
                                        AbstractOrderByExpr::VectorDistance(
                                            ColumnPath::Physical(new_column_path),
                                            ColumnPath::Param(SQLParamContainer::f32_array(
                                                vector_value,
                                            )),
                                            parameter
                                                .vector_distance_function
                                                .unwrap_or(VectorDistanceFunction::default()),
                                        ),
                                        ordering,
                                    )])
                                })
                            }
                            _ => Err(PostgresExecutionError::Validation(
                                parameter_name.into(),
                                "Invalid vector order by parameter".into(),
                            )),
                        },
                        OrderByParameterTypeKind::Composite { .. } => {
                            map_order_by(
                                parameter,
                                parameter_value,
                                new_column_path,
                                order_by_types,
                                request_context,
                                access_checker,
                            )
                            .await
                        }
                    }
                }
                None => Err(PostgresExecutionError::Validation(
                    parameter_name.into(),
                    "Invalid order by parameter".into(),
                )),
            }
        }
        _ => Err(PostgresExecutionError::Validation(
            parameter_name.into(),
            "Invalid primitive or vector order by parameter".into(),
        )),
    }
}

fn ordering(argument: &Val) -> Result<Ordering, PostgresExecutionError> {
    fn str_ordering(value: &str) -> Result<Ordering, PostgresExecutionError> {
        if value == "ASC" {
            Ok(Ordering::Asc)
        } else if value == "DESC" {
            Ok(Ordering::Desc)
        } else {
            Err(PostgresExecutionError::Generic(format!(
                "Cannot match {value} as valid ordering",
            )))
        }
    }

    match argument {
        Val::Enum(value) => str_ordering(value.as_str()),
        Val::String(value) => str_ordering(value.as_str()), // Needed when processing values from variables (that don't get mapped to the Enum type)
        arg => Err(PostgresExecutionError::Generic(format!(
            "Unable to process ordering argument {arg}",
        ))),
    }
}

/// Entry point for computing order-by with field-level access checking.
///
/// This is the primary API for both GraphQL and RPC resolvers. It uses the
/// subsystem's access expressions to enforce field-level access control.
pub async fn compute_order_by<'a>(
    param: &'a OrderByParameter,
    argument: &'a Val,
    subsystem: &'a PostgresCoreSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractOrderBy, PostgresExecutionError> {
    let access_checker = CoreOrderByFieldAccessChecker { subsystem };

    map_order_by(
        param,
        argument,
        None,
        &subsystem.order_by_types,
        request_context,
        &access_checker,
    )
    .await
}
