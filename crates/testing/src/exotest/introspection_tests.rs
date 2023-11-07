// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{anyhow, Result};
use colored::Colorize;

use core_resolver::{context::RequestContext, system_resolver::SystemResolver, OperationsPayload};
use exo_deno::{
    deno_core::{url::Url, ModuleType},
    deno_error::DenoError,
    deno_executor_pool::{DenoScriptDefn, ResolvedModule},
    Arg, DenoModule, DenoModuleSharedState, UserCode,
};
use exo_sql::{LOCAL_CHECK_CONNECTION_ON_STARTUP, LOCAL_CONNECTION_POOL_SIZE, LOCAL_URL};
use include_dir::{include_dir, Dir};
use resolver::{create_system_resolver, LOCAL_ALLOW_INTROSPECTION};
use serde_json::Value;
use std::{collections::HashMap, path::Path};

use crate::exotest::common::TestResultKind;

use super::{
    common::TestResult,
    integration_tests::{run_query, MemoryRequest},
};

const INTROSPECTION_ASSERT_JS: &str = include_str!("introspection_tests.js");
const GRAPHQL_NODE_MODULE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/node_modules/graphql");

pub(crate) async fn run_introspection_test(model_path: &Path) -> Result<TestResult> {
    let log_prefix = format!("(introspection: {})\n :: ", model_path.display()).purple();
    println!("{log_prefix} Running introspection tests...");

    let exo_ir_file = format!("{}/target/index.exo_ir", model_path.display()).to_string();

    let server = {
        let static_loaders = server_common::create_static_loaders();

        LOCAL_URL
            .with(|url| {
                url.borrow_mut()
                    .replace("postgres://a@dummy-value".to_string());

                LOCAL_CONNECTION_POOL_SIZE.with(|pool_size| {
                    pool_size.borrow_mut().replace(1);

                    LOCAL_ALLOW_INTROSPECTION.with(|allow| {
                        allow.borrow_mut().replace(true);

                        LOCAL_CHECK_CONNECTION_ON_STARTUP.with(|check_connection| {
                            check_connection.borrow_mut().replace(false);

                            create_system_resolver(&exo_ir_file, static_loaders)
                        })
                    })
                })
            })
            .await?
    };

    let result = check_introspection(&server).await?;

    match result {
        Ok(()) => Ok(TestResult {
            log_prefix: log_prefix.to_string(),
            result: TestResultKind::Success,
        }),

        Err(e) => Ok(TestResult {
            log_prefix: log_prefix.to_string(),
            result: TestResultKind::Fail(e),
        }),
    }
}

async fn check_introspection(server: &SystemResolver) -> Result<Result<()>> {
    let script = INTROSPECTION_ASSERT_JS;

    let mut deno_module = DenoModule::new(
        UserCode::LoadFromMemory {
            path: "file://internal/introspection_tests.js".to_owned(),
            script: DenoScriptDefn {
                modules: vec![(
                    Url::parse("file://internal/introspection_tests.js").unwrap(),
                    ResolvedModule::Module(
                        script.into(),
                        ModuleType::JavaScript,
                        Url::parse("file://internal/introspection_tests.js").unwrap(),
                        false,
                    ),
                )]
                .into_iter()
                .collect(),
                npm_snapshot: None,
            },
        },
        "ExographTest",
        vec![],
        vec![],
        vec![],
        DenoModuleSharedState::default(),
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
    .await?;

    let query = deno_module
        .execute_function("introspectionQuery", vec![])
        .await?;

    let request = MemoryRequest::new(HashMap::new());
    let request_context = RequestContext::new(&request, vec![], server)?;
    let operations_payload = OperationsPayload {
        operation_name: None,
        query: if let Value::String(s) = query {
            s
        } else {
            panic!("expected string")
        },
        variables: None,
    };

    let result = run_query(
        operations_payload,
        request_context,
        server,
        &mut HashMap::new(),
    )
    .await;

    let result = deno_module
        .execute_function(
            "assertSchema",
            vec![Arg::Serde(Value::String(result.to_string()))],
        )
        .await;

    match result {
        Ok(_) => Ok(Ok(())),
        Err(e) => match e {
            DenoError::Explicit(e) => Ok(Err(anyhow!(e))),
            e => Err(anyhow!(e)),
        },
    }
}
