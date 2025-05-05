// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{plugin::WasmSubsystemResolver, wasm_execution_error::WasmExecutionError};
use common::context::RequestContext;
use common::value::Val as ExoVal;
use core_resolver::{
    system_resolver::GraphQLSystemResolver, validation::field::ValidatedField, QueryResponse,
    QueryResponseBody,
};
use std::collections::HashMap;
use wasm_graphql_model::module::ModuleMethod;
use wasmtime::Val;

pub struct WasmOperation<'a> {
    pub method: &'a ModuleMethod,
    pub field: &'a ValidatedField,
    #[allow(unused)]
    pub request_context: &'a RequestContext<'a>,
    pub subsystem_resolver: &'a WasmSubsystemResolver,
    #[allow(unused)]
    pub system_resolver: &'a GraphQLSystemResolver,
}

impl WasmOperation<'_> {
    pub async fn execute(&self) -> Result<QueryResponse, WasmExecutionError> {
        let script = &self.subsystem_resolver.subsystem.scripts[self.method.script];

        let mapped_args: HashMap<String, Val> = self
            .field
            .arguments
            .iter()
            .map(|(gql_name, gql_value)| {
                (
                    gql_name.as_str().to_owned(),
                    match gql_value {
                        ExoVal::Null => todo!(),
                        ExoVal::Number(num) => (num.as_i64().unwrap() as i32).into(),
                        ExoVal::String(_) => todo!(),
                        ExoVal::Bool(_) => todo!(),
                        ExoVal::Binary(_) => todo!(),
                        ExoVal::Enum(_) => todo!(),
                        ExoVal::List(_) => todo!(),
                        ExoVal::Object(_) => todo!(),
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
                    *val
                } else {
                    todo!()
                }
            })
            .collect();

        let result = self
            .subsystem_resolver
            .executor
            .execute(&script.path, &script.script, &self.method.name, args)
            .await
            .map_err(WasmExecutionError::Wasm)?;

        Ok(QueryResponse {
            body: QueryResponseBody::Json(result),
            headers: vec![], // TODO: support headers
        })
    }
}
