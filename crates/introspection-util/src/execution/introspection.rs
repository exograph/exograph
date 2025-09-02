// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use exo_deno::{
    Arg, DenoModule, UserCode,
    deno_core::{ModuleType, url::Url},
    deno_executor_pool::{DenoScriptDefn, ResolvedModule},
};
use include_dir::{Dir, include_dir};
use serde_json::Value;

const INTROSPECTION_ASSERT_JS: &str = include_str!("introspection.js");
const GRAPHQL_NODE_MODULE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/node_modules/graphql");

pub async fn get_introspection_query() -> Result<Value> {
    execute_introspection_deno_function("introspectionQuery", vec![]).await
}

pub async fn schema_sdl(schema_response: Value) -> Result<String> {
    let sdl =
        execute_introspection_deno_function("schemaSDL", vec![Arg::Serde(schema_response)]).await?;

    if let Value::String(s) = sdl {
        Ok(s)
    } else {
        Err(anyhow!("expected string"))
    }
}

pub async fn create_introspection_deno_module() -> Result<DenoModule> {
    let script = INTROSPECTION_ASSERT_JS;

    let script_path = "file://internal/introspection.js";

    DenoModule::new(
        UserCode::LoadFromMemory {
            path: script_path.to_owned(),
            script: DenoScriptDefn {
                modules: vec![(
                    Url::parse(script_path).unwrap(),
                    ResolvedModule::Module(
                        script.into(),
                        ModuleType::JavaScript,
                        Url::parse(script_path).unwrap(),
                        false,
                    ),
                )]
                .into_iter()
                .collect(),
            },
        },
        vec![],
        vec![],
        vec![],
        Some("Error"),
        Some(HashMap::from([(
            "graphql".to_string(),
            &GRAPHQL_NODE_MODULE,
        )])),
        Some(vec![(
            // TODO: move to a Rust-based solution
            // maybe juniper: https://github.com/graphql-rust/juniper/issues/217

            // We are currently importing the `graphql` NPM module used by graphiql and running it through Deno to perform schema validation
            // As it only depends on deno_core and deno_runtime, our integration of Deno does not include the NPM implementation provided through deno_cli
            // Therefore, we need to patch certain things in this module through extra_sources to get scripts to run in Deno

            // ReferenceError: process is not defined
            //    at embedded://graphql/jsutils/instanceOf.mjs:11:16
            "embedded://graphql/jsutils/instanceOf.mjs",
            GRAPHQL_NODE_MODULE
                .get_file("jsutils/instanceOf.mjs")
                .unwrap()
                .contents_utf8()
                .unwrap()
                .replace("process.env.NODE_ENV === 'production'", "false"),
        )]),
    )
    .await
}

pub async fn execute_introspection_deno_function(
    function_name: &str,
    args: Vec<Arg>,
) -> Result<Value> {
    let function_name = function_name.to_string();

    tokio::task::spawn_blocking({
        move || {
            tokio::runtime::Handle::current().block_on(async move {
                let mut deno_module = create_introspection_deno_module().await?;
                deno_module
                    .execute_function(&function_name, args)
                    .await
                    .map_err(|e| anyhow!("Error executing function: {:?}", e))
            })
        }
    })
    .await?
}
