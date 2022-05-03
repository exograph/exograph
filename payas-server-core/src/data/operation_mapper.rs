use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use payas_deno::Arg;
use payas_sql::{
    AbstractInsert, AbstractOperation, AbstractPredicate, AbstractSelect, AbstractUpdate,
};
use serde_json::{Map, Value};
use tokio_postgres::{types::FromSqlOwned, Row};

use crate::{
    execution::operations_context::{OperationsContext, QueryResponse},
    validation::field::ValidatedField,
    OperationsPayload,
};

use super::access_solver;
use super::interception::InterceptedOperation;
use crate::execution::resolver::{FieldResolver, GraphQLExecutionError};
use async_graphql_value::ConstValue;
use payas_model::model::{
    mapped_arena::SerializableSlabIndex,
    operation::{Interceptors, Mutation, OperationReturnType},
    service::{ServiceMethod, ServiceMethodType},
    GqlCompositeType, GqlCompositeTypeKind, GqlTypeKind,
};
use payas_sql::Predicate;

pub trait SQLMapper<'a, R> {
    fn map_to_sql(
        &'a self,
        argument: &'a ConstValue,
        query_context: &'a OperationsContext<'a>,
    ) -> Result<R>;
}

pub trait SQLInsertMapper<'a> {
    fn insert_operation(
        &'a self,
        mutation: &'a Mutation,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        query_context: &'a OperationsContext<'a>,
    ) -> Result<AbstractInsert>;
}

pub trait SQLUpdateMapper<'a> {
    fn update_operation(
        &'a self,
        mutation: &'a Mutation,
        predicate: AbstractPredicate<'a>,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        query_context: &'a OperationsContext<'a>,
    ) -> Result<AbstractUpdate>;
}

#[async_trait]
pub trait OperationResolver<'a> {
    fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        query_context: &'a OperationsContext<'a>,
    ) -> Result<OperationResolverResult<'a>>;

    async fn execute(
        &'a self,
        field: &'a ValidatedField,
        query_context: &'a OperationsContext<'a>,
    ) -> Result<QueryResponse> {
        let resolver_result = self.resolve_operation(field, query_context)?;
        let interceptors = self.interceptors().ordered();

        let op_name = &self.name();

        let intercepted_operation =
            InterceptedOperation::new(op_name, resolver_result, interceptors);
        intercepted_operation.execute(field, query_context).await
    }

    fn name(&self) -> &str;

    fn interceptors(&self) -> &Interceptors;
}

#[allow(clippy::large_enum_variant)]
pub enum OperationResolverResult<'a> {
    SQLOperation(AbstractOperation<'a>),
    DenoOperation(SerializableSlabIndex<ServiceMethod>),
}

pub enum SQLOperationKind {
    Create,
    Retrieve,
    Update,
    Delete,
}

pub fn compute_sql_access_predicate<'a>(
    return_type: &OperationReturnType,
    kind: &SQLOperationKind,
    query_context: &'a OperationsContext<'a>,
) -> AbstractPredicate<'a> {
    let return_type = return_type.typ(query_context.get_system());

    match &return_type.kind {
        GqlTypeKind::Primitive => AbstractPredicate::True,
        GqlTypeKind::Composite(GqlCompositeType { access, .. }) => {
            let access_expr = match kind {
                SQLOperationKind::Create => &access.creation,
                SQLOperationKind::Retrieve => &access.read,
                SQLOperationKind::Update => &access.update,
                SQLOperationKind::Delete => &access.delete,
            };
            access_solver::solve_access(
                access_expr,
                query_context.request_context,
                query_context.system,
            )
        }
    }
}

pub fn compute_service_access_predicate<'a>(
    return_type: &OperationReturnType,
    method: &'a ServiceMethod,
    query_context: &'a OperationsContext<'a>,
) -> &'a Predicate<'a> {
    let return_type = return_type.typ(query_context.get_system());

    let type_level_access = match &return_type.kind {
        GqlTypeKind::Primitive => Predicate::True,
        GqlTypeKind::Composite(GqlCompositeType {
            access,
            kind: GqlCompositeTypeKind::NonPersistent,
            ..
        }) => {
            let access_expr = match &method.operation_kind {
                ServiceMethodType::Query(_) => &access.read, // query
                ServiceMethodType::Mutation(_) => &access.creation, // mutation
            };

            access_solver::solve_access(
                access_expr,
                query_context.request_context,
                query_context.system,
            )
        }
        _ => panic!(),
    };

    let method_access_expr = match &method.operation_kind {
        ServiceMethodType::Query(_) => &method.access.read, // query
        ServiceMethodType::Mutation(_) => &method.access.creation, // mutation
    };

    let method_level_access = access_solver::solve_access(
        method_access_expr,
        query_context.request_context,
        query_context.system,
    );
    let method_level_access = method_level_access.predicate();

    if matches!(type_level_access, AbstractPredicate::False)
        || matches!(method_level_access, Predicate::False)
    {
        &Predicate::False // deny if either access check fails
    } else {
        &Predicate::True
    }
}

impl<'a> OperationResolverResult<'a> {
    pub async fn execute(
        &self,
        field: &'a ValidatedField,
        query_context: &'a OperationsContext<'a>,
    ) -> Result<QueryResponse> {
        match self {
            OperationResolverResult::SQLOperation(abstract_operation) => {
                let mut result = query_context
                    .executor
                    .database_executor
                    .execute(abstract_operation)
                    .await?;

                if result.len() == 1 {
                    let string_result = extractor(result.swap_remove(0))?;
                    Ok(QueryResponse::Raw(Some(string_result)))
                } else if result.is_empty() {
                    Ok(QueryResponse::Raw(None))
                } else {
                    bail!(format!(
                        "Result has {} entries; expected only zero or one",
                        result.len()
                    ))
                }
            }

            OperationResolverResult::DenoOperation(method_id) => {
                let method = &query_context.system.methods[*method_id];

                let access_predicate =
                    compute_service_access_predicate(&method.return_type, method, query_context);

                if access_predicate == &Predicate::False {
                    bail!(anyhow!(GraphQLExecutionError::Authorization))
                }

                resolve_deno(method, field, query_context)
                    .await
                    .map(QueryResponse::Json)
            }
        }
    }
}

async fn resolve_deno(
    method: &ServiceMethod,
    field: &ValidatedField,
    query_context: &OperationsContext<'_>,
) -> Result<serde_json::Value> {
    let script = &query_context.system.deno_scripts[method.script];
    let system = query_context.get_system();

    let mapped_args = field
        .arguments
        .iter()
        .map(|(gql_name, gql_value)| {
            (
                gql_name.as_str().to_owned(),
                gql_value.clone().into_json().unwrap(),
            )
        })
        .collect::<HashMap<_, _>>();

    // construct a sequence of arguments to pass to the Deno method
    let arg_sequence = method
        .arguments
        .iter()
        .map(|arg| {
            if arg.is_injected {
                // handle injected arguments

                let arg_type = &system.types[arg.type_id];

                // what kind of injected argument is it?
                // first check if it's a context
                if let Some(context) = system
                    .contexts
                    .iter()
                    .map(|(_, context)| context)
                    .find(|context| context.name == arg_type.name)
                {
                    // this argument is a context, get the value of the context and give it as an argument
                    let context_value = query_context
                        .request_context
                        .get(&context.name)
                        .unwrap_or_else(|| {
                            panic!(
                                "Could not get context `{}` from request context",
                                &context.name
                            )
                        });
                    Ok(Arg::Serde(context_value.clone()))
                } else {
                    // not a context, assume it is a provided shim by the Deno executor
                    Ok(Arg::Shim(arg_type.name.clone()))
                }
            } else if let Some(val) = mapped_args.get(&arg.name) {
                // regular argument
                Ok(Arg::Serde(val.clone()))
            } else {
                Err(anyhow!("Invalid argument {}", arg.name))
            }
        })
        .collect::<Result<Vec<_>>>()?;

    query_context
        .executor
        .deno_execution
        .preload_module(&script.path, &script.script, 1)
        .await?;

    let function_result = query_context
        .executor
        .deno_execution
        .execute_function_with_shims(
            &script.path,
            &script.script,
            &method.name,
            arg_sequence,
            Some(
                &|query_string: String, variables: Option<Map<String, Value>>| {
                    Box::pin(async move {
                        let result = query_context
                            .executor
                            .execute_with_request_context(
                                OperationsPayload {
                                    operation_name: None,
                                    query: query_string,
                                    variables,
                                },
                                query_context.request_context.clone(),
                            )
                            .await?
                            .into_iter()
                            .map(|(name, response)| (name, response.to_json().unwrap()))
                            .collect::<Map<_, _>>();

                        Ok(serde_json::Value::Object(result))
                    })
                },
            ),
            None,
            None,
        )
        .await?;

    let result = if let serde_json::Value::Object(_) = function_result {
        let resolved_set = function_result
            .resolve_fields(query_context, &field.subfields)
            .await?;
        serde_json::Value::Object(resolved_set.into_iter().collect())
    } else {
        function_result
    };

    Ok(result)
}

fn extractor<T: FromSqlOwned>(row: Row) -> Result<T> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => bail!("Got row without any columns {}", err),
    }
}

// TODO: Define this so that we can use it from multiple ways to invoke (service method and interceptors)
// fn execute_query_fn<'a>(
//     query_context: &'a QueryContext<'a>,
// ) -> &'a dyn Fn(
//     String,
//     Option<&serde_json::Map<String, serde_json::Value>>,
// ) -> Result<serde_json::Value> {
//     &|query_string: String, variables| {
//         let result = query_context
//             .executor
//             .execute_with_request_context(
//                 None,
//                 &query_string,
//                 variables,
//                 query_context.request_context.clone(),
//             )?
//             .into_iter()
//             .map(|(name, response)| (name, response.to_json().unwrap()))
//             .collect::<Map<_, _>>();

//         Ok(serde_json::Value::Object(result))
//     }
// }
