use std::collections::HashMap;

use crate::{
    data::operation_mapper::OperationResolverResult,
    execution::{
        query_context::{QueryContext, QueryResponse},
        resolver::{FieldResolver, GraphQLExecutionError},
    },
};
use anyhow::{anyhow, bail, Result};
use async_graphql_parser::{
    types::{Field, OperationType},
    Positioned,
};

use payas_deno::Arg;
use payas_model::{model::system::ModelSystem, sql::predicate::Predicate};
use postgres::{types::FromSqlOwned, Row};
use serde_json::Value;

use super::{
    operation_context::OperationContext,
    operation_mapper::{compute_service_access_predicate, OperationResolver},
};

pub trait DataResolver {
    fn resolve(
        &self,
        field: &Positioned<Field>,
        operation_type: &OperationType,
        query_context: &QueryContext<'_>,
    ) -> Result<QueryResponse>;
}

impl FieldResolver<Value> for Value {
    fn resolve_field<'a>(
        &'a self,
        _query_context: &QueryContext<'_>,
        field: &Positioned<Field>,
    ) -> Result<Value> {
        let field_name = field.node.name.node.as_str();

        if let Value::Object(map) = self {
            map.get(field_name)
                .cloned()
                .ok_or_else(|| anyhow!("No field named {} in Object", field_name))
        } else {
            Err(anyhow!(
                "{} is not an Object and doesn't have any fields",
                field_name
            ))
        }
    }
}

impl DataResolver for ModelSystem {
    fn resolve(
        &self,
        field: &Positioned<Field>,
        operation_type: &OperationType,
        query_context: &QueryContext<'_>,
    ) -> Result<QueryResponse> {
        let operation_context = OperationContext::new(query_context);

        let resolver_result = match operation_type {
            OperationType::Query => {
                let operation = self.queries.get_by_key(&field.node.name.node);
                operation
                    .unwrap()
                    .resolve_operation(field, &operation_context)
            }
            OperationType::Mutation => {
                let operation = self.create_mutations.get_by_key(&field.node.name.node);
                operation
                    .unwrap()
                    .resolve_operation(field, &operation_context)
            }
            OperationType::Subscription => {
                todo!()
            }
        }?;

        match resolver_result {
            OperationResolverResult::SQLOperation(transaction_script) => {
                let mut client = query_context.executor.database.get_client()?;
                let mut result = transaction_script.execute(&mut client, extractor)?;

                if result.len() == 1 {
                    Ok(QueryResponse::Raw(Some(result.swap_remove(0))))
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
                let method = &query_context.executor.system.methods[method_id];
                let path = &method.module_path;

                let access_predicate = compute_service_access_predicate(
                    &method.return_type,
                    method,
                    &operation_context,
                );

                if access_predicate == &Predicate::False {
                    bail!(anyhow!(GraphQLExecutionError::Authorization))
                }

                let mut deno_modules_map = query_context.executor.deno_modules_map.lock().unwrap();
                let function_result = futures::executor::block_on(async {
                    let mapped_args = field
                        .node
                        .arguments
                        .iter()
                        .map(|(gql_name, gql_value)| {
                            (
                                gql_name.node.as_str().to_owned(),
                                gql_value.node.clone().into_json().unwrap(),
                            )
                        })
                        .collect::<HashMap<_, _>>();

                    let arg_sequence = method
                        .arguments
                        .iter()
                        .map(|arg| {
                            let arg_type = &query_context.executor.system.types[arg.type_id];

                            if arg.is_injected {
                                Ok(Arg::Shim(arg_type.name.clone()))
                            } else if let Some(val) = mapped_args.get(&arg.name) {
                                Ok(Arg::Serde(val.clone()))
                            } else {
                                Err(anyhow!("Invalid argument {}", arg.name))
                            }
                        })
                        .collect::<Result<Vec<_>>>()?;

                    deno_modules_map.load_module(path)?;
                    deno_modules_map.execute_function(path, &method.name, arg_sequence)
                })?;

                let result = if let Value::Object(_) = function_result {
                    let resolved_set = function_result
                        .resolve_selection_set(query_context, &field.node.selection_set)?;
                    Value::Object(resolved_set.into_iter().collect())
                } else {
                    function_result
                };

                Ok(QueryResponse::Json(result))
            }
        }
    }
}

pub fn extractor<T: FromSqlOwned>(row: Row) -> Result<T> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => bail!("Got row without any columns {}", err),
    }
}
