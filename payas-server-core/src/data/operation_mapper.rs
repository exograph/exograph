use std::collections::HashMap;

use crate::deno_integration::{ClayCallbackProcessor, FnClaytipExecuteQuery};
use crate::execution::operations_context::QueryResponseBody;
use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use futures::FutureExt;
use futures::StreamExt;
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
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        query_context: &'a OperationsContext<'a>,
    ) -> Result<OperationResolverResult<'a>>;

    async fn execute(
        &'a self,
        field: &'a ValidatedField,
        query_context: &'a OperationsContext<'a>,
    ) -> Result<QueryResponse> {
        let resolver_result = self.resolve_operation(field, query_context).await?;
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

pub async fn compute_sql_access_predicate<'a>(
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
            .await
        }
    }
}

pub async fn compute_service_access_predicate<'a>(
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
            .await
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
    )
    .await;

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

                let body: Result<QueryResponseBody> = if result.len() == 1 {
                    let string_result = extractor(result.swap_remove(0))?;
                    Ok(QueryResponseBody::Raw(Some(string_result)))
                } else if result.is_empty() {
                    Ok(QueryResponseBody::Raw(None))
                } else {
                    bail!(format!(
                        "Result has {} entries; expected only zero or one",
                        result.len()
                    ))
                };

                Ok(QueryResponse {
                    body: body?,
                    headers: vec![], // we shouldn't get any HTTP headers from a SQL op
                })
            }

            OperationResolverResult::DenoOperation(method_id) => {
                let method = &query_context.system.methods[*method_id];

                let access_predicate =
                    compute_service_access_predicate(&method.return_type, method, query_context)
                        .await;

                if access_predicate == &Predicate::False {
                    bail!(anyhow!(GraphQLExecutionError::Authorization))
                }

                resolve_deno(
                    method,
                    field,
                    super::claytip_execute_query!(query_context),
                    query_context,
                )
                .await
            }
        }
    }
}

async fn resolve_deno<'a>(
    method: &ServiceMethod,
    field: &ValidatedField,
    claytip_execute_query: Option<&'a FnClaytipExecuteQuery<'a>>,
    query_context: &OperationsContext<'_>,
) -> Result<QueryResponse> {
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
    let arg_sequence: Vec<Arg> = futures::stream::iter(method.arguments.iter())
        .then(|arg| async {
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
                        .extract_context(context)
                        .await
                        .unwrap_or_else(|_| {
                            panic!(
                                "Could not get context `{}` from request context",
                                &context.name
                            )
                        });
                    Ok(Arg::Serde(context_value))
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
        .collect::<Vec<Result<_>>>()
        .await
        .into_iter()
        .collect::<Result<_>>()?;

    let callback_processor = ClayCallbackProcessor {
        claytip_execute_query,
        claytip_proceed: None,
    };

    let (result, response) = query_context
        .executor
        .deno_execution_pool
        .execute_and_get_r(
            &script.path,
            &script.script,
            &method.name,
            arg_sequence,
            None,
            callback_processor,
        )
        .await?;

    let result = if let serde_json::Value::Object(_) = result {
        let resolved_set = result
            .resolve_fields(query_context, &field.subfields)
            .await?;
        serde_json::Value::Object(resolved_set.into_iter().collect())
    } else {
        result
    };

    Ok(QueryResponse {
        body: QueryResponseBody::Json(result),
        headers: response.map(|r| r.headers).unwrap_or_default(),
    })
}

fn extractor<T: FromSqlOwned>(row: Row) -> Result<T> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => bail!("Got row without any columns {}", err),
    }
}
