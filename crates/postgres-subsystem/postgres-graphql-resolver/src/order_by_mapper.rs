// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use futures::future::join_all;

use crate::{auth_util::check_retrieve_access, sql_mapper::SQLMapper};
use common::context::RequestContext;
use common::value::Val;
use exo_sql::{
    AbstractOrderBy, AbstractOrderByExpr, AbstractPredicate, Ordering, PhysicalColumnPath,
};
use postgres_core_resolver::column_path_util::to_column_path;
use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;
use postgres_core_resolver::predicate_util::to_pg_vector;

use exo_sql::{ColumnPath, SQLParamContainer, VectorDistanceFunction};

use postgres_graphql_model::{
    order::{OrderByParameter, OrderByParameterType, OrderByParameterTypeKind},
    subsystem::PostgresGraphQLSubsystem,
};

pub(crate) struct OrderByParameterInput<'a> {
    pub param: &'a OrderByParameter,
    pub parent_column_path: Option<PhysicalColumnPath>,
}

#[async_trait]
impl<'a> SQLMapper<'a, AbstractOrderBy> for OrderByParameterInput<'a> {
    async fn to_sql(
        self,
        argument: &'a Val,
        subsystem: &'a PostgresGraphQLSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractOrderBy, PostgresExecutionError> {
        let parameter_type = &subsystem.order_by_types[self.param.typ.innermost().type_id];
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
                        self.parent_column_path.clone(),
                        subsystem,
                        request_context,
                    )
                });

                let mapped = join_all(mapped).await.into_iter().collect();

                flatten(mapped)
            }
            Val::List(elems) => {
                let mapped = elems.iter().map(|elem| {
                    OrderByParameterInput {
                        param: self.param,
                        parent_column_path: self.parent_column_path.clone(),
                    }
                    .to_sql(elem, subsystem, request_context)
                });

                let mapped = join_all(mapped).await.into_iter().collect();
                flatten(mapped)
            }

            _ => Err(PostgresExecutionError::Validation(
                self.param.name.clone(),
                format!("Invalid argument ('{argument}')"),
            )),
        }
    }

    fn param_name(&self) -> &str {
        &self.param.name
    }
}

async fn order_by_pair<'a>(
    typ: &'a OrderByParameterType,
    parameter_name: &str,
    parameter_value: &'a Val,
    parent_column_path: Option<PhysicalColumnPath>,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractOrderBy, PostgresExecutionError> {
    match &typ.kind {
        OrderByParameterTypeKind::Composite { parameters } => {
            match parameters.iter().find(|p| p.name == parameter_name) {
                Some(parameter) => {
                    let field_access = match parameter.access {
                        Some(ref access) => {
                            check_retrieve_access(
                                &subsystem.core_subsystem.database_access_expressions[access.read],
                                subsystem,
                                request_context,
                            )
                            .await?
                        }
                        None => AbstractPredicate::True,
                    };

                    if field_access != AbstractPredicate::True {
                        return Err(PostgresExecutionError::Authorization);
                    }

                    let base_param_type =
                        &subsystem.order_by_types[parameter.typ.innermost().type_id];

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
                            OrderByParameterInput {
                                param: parameter,
                                parent_column_path: new_column_path,
                            }
                            .to_sql(parameter_value, subsystem, request_context)
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
