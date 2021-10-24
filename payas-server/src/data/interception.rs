use async_graphql_parser::{types::Field, Positioned};
use payas_deno::Arg;
use payas_model::model::interceptor::{Interceptor, InterceptorKind};

use crate::execution::query_context::{QueryContext, QueryResponse};
use anyhow::{bail, Result};
use serde_json::json;
use serde_json::Map;

use super::{operation_context::OperationContext, operation_mapper::OperationResolverResult};

// enum InterceptionTree<'a> {
//     Leaf(OperationResolverResult<'a>),
//     Branch(Box<InterceptionTree<'a>>),
// }

pub struct InterceptionOperation<'a> {
    operation_name: &'a str,
    resolver_result: OperationResolverResult<'a>,
    interceptors: Vec<&'a Interceptor>,
}

impl<'a> InterceptionOperation<'a> {
    pub fn new(
        operation_name: &'a str,
        resolver_result: OperationResolverResult<'a>,
        interceptors: Vec<&'a Interceptor>,
    ) -> Self {
        Self {
            operation_name,
            resolver_result,
            interceptors,
        }
    }

    pub fn execute(
        self,
        field: &'a Positioned<Field>,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<QueryResponse> {
        match self.interceptors.split_first() {
            Some((head, tail)) => {
                let sub = InterceptionOperation::new(
                    self.operation_name,
                    self.resolver_result,
                    tail.to_vec(),
                );

                if head.interceptor_kind == InterceptorKind::Before {
                    Self::execute_interceptor(
                        self.operation_name,
                        head,
                        operation_context.query_context,
                    )?;

                    sub.execute(field, operation_context)
                } else {
                    let res = sub.execute(field, operation_context);
                    Self::execute_interceptor(
                        self.operation_name,
                        head,
                        operation_context.query_context,
                    )?;

                    res
                }
            }
            None => self.resolver_result.execute(field, operation_context),
        }
    }

    fn execute_interceptor(
        operation_name: &'a str,
        interceptor: &Interceptor,
        query_context: &QueryContext<'_>,
    ) -> Result<()> {
        let path = &interceptor.module_path;

        let mut deno_modules_map = query_context.executor.deno_modules_map.lock().unwrap();

        let arg_sequence = interceptor
            .arguments
            .iter()
            .map(|arg| {
                let arg_type = &query_context.executor.system.types[arg.type_id];

                if arg_type.name == "Operation" {
                    Ok(Arg::Serde(json!({ "name": operation_name })))
                } else if arg_type.name == "ClaytipInjected" {
                    // TODO: Change this to supply a shim if the arg_type is one of the shimmable types
                    Ok(Arg::Shim(arg_type.name.clone()))
                } else {
                    bail!("Invalid argument type {}", arg_type.name)
                }
            })
            .collect::<Result<Vec<_>>>()?;

        deno_modules_map.load_module(path)?;
        deno_modules_map
            .execute_function(
                path,
                &interceptor.name,
                arg_sequence,
                // TODO: This block is duplicate of that from resolve_deno()
                &|query_string, variables| {
                    let result = query_context
                        .executor
                        .execute_with_request_context(
                            None,
                            &query_string,
                            variables,
                            query_context.request_context.clone(),
                        )?
                        .into_iter()
                        .map(|(name, response)| (name, response.to_json().unwrap()))
                        .collect::<Map<_, _>>();

                    Ok(serde_json::Value::Object(result))
                },
            )
            .map(|_| ())
    }
}
