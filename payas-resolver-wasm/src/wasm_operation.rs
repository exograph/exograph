use std::collections::HashMap;

use async_graphql_value::ConstValue;
use payas_deno_model::service::ServiceMethod;
use payas_resolver_core::{
    request_context::RequestContext, validation::field::ValidatedField, QueryResponse,
    QueryResponseBody,
};
use wasmtime::Val;

use crate::{wasm_execution_error::WasmExecutionError, wasm_system_context::WasmSystemContext};

pub struct WasmOperation<'a> {
    pub method: &'a ServiceMethod,
    pub field: &'a ValidatedField,
    pub request_context: &'a RequestContext<'a>,
}

impl<'a> WasmOperation<'a> {
    pub async fn execute(
        &self,
        wasm_system_context: &WasmSystemContext<'a>,
    ) -> Result<QueryResponse, WasmExecutionError> {
        let script = &wasm_system_context.system.scripts[self.method.script];

        let mapped_args: HashMap<String, Val> = self
            .field
            .arguments
            .iter()
            .map(|(gql_name, gql_value)| {
                (
                    gql_name.as_str().to_owned(),
                    match gql_value {
                        ConstValue::Null => todo!(),
                        ConstValue::Number(num) => (num.as_i64().unwrap() as i32).into(),
                        ConstValue::String(_) => todo!(),
                        ConstValue::Boolean(_) => todo!(),
                        ConstValue::Binary(_) => todo!(),
                        ConstValue::Enum(_) => todo!(),
                        ConstValue::List(_) => todo!(),
                        ConstValue::Object(_) => todo!(),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        let args: Vec<_> = self
            .method
            .arguments
            .iter()
            .map(|arg| {
                if let Some(val) = mapped_args.get(&arg.name) {
                    val.clone()
                } else {
                    todo!()
                }
            })
            .collect();

        let result = wasm_system_context
            .executor_pool
            .execute(&script.path, &script.script, &self.method.name, args)
            .await
            .map_err(WasmExecutionError::Wasm)?;

        Ok(QueryResponse {
            body: QueryResponseBody::Json(result),
            headers: vec![], // TODO: support headers
        })
    }
}
