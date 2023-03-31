use anyhow::{anyhow, Result};
use include_dir::{include_dir, Dir};
use payas_deno::{deno_error::DenoError, Arg, DenoModule, DenoModuleSharedState, UserCode};
use serde_json::Value;
use std::{collections::HashMap, path::Path};

use crate::exotest::{
    common::{spawn_exo_server, TestResultKind},
    integration_tests::build_exo_ir_file,
};

use super::common::TestResult;

const INTROSPECTION_ASSERT_JS: &str = include_str!("introspection_tests.js");
const GRAPHQL_NODE_MODULE: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/../../graphiql/node_modules/graphql");

pub(crate) fn run_introspection_test(model_path: &Path) -> Result<TestResult> {
    let log_prefix =
        ansi_term::Color::Purple.paint(format!("(introspection: {})\n :: ", model_path.display()));
    println!("{log_prefix} Running introspection tests...");

    build_exo_ir_file(model_path)?;

    let server = spawn_exo_server(
        model_path,
        [
            ("EXO_INTROSPECTION", "true"),
            ("EXO_POSTGRES_URL", "postgres://a@dummy-value"),
            ("EXO_CHECK_CONNECTION_ON_STARTUP", "false"),
            ("EXO_SERVER_PORT", "0"), // ask exo-server to select a free port
        ]
        .into_iter(),
    )?;

    let result = check_introspection(&server.endpoint)?;
    let output: String = server.output.lock().unwrap().clone();

    match result {
        Ok(()) => Ok(TestResult {
            log_prefix: log_prefix.to_string(),
            result: TestResultKind::Success,
            output,
        }),

        Err(e) => Ok(TestResult {
            log_prefix: log_prefix.to_string(),
            result: TestResultKind::Fail(e),
            output,
        }),
    }
}

fn check_introspection(endpoint: &str) -> Result<Result<()>> {
    println!(
        "resolver/build.rs cwd = {:?} {:?}",
        std::env::current_dir().unwrap(),
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );

    let script = INTROSPECTION_ASSERT_JS;

    let deno_module_future = DenoModule::new(
        UserCode::LoadFromMemory {
            path: "internal/introspection_tests.js".to_owned(),
            script: script.into(),
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
    );

    let runtime = tokio::runtime::Runtime::new()?;
    let mut deno_module = runtime.block_on(deno_module_future)?;

    let result = runtime.block_on(deno_module.execute_function(
        "assertSchema",
        vec![Arg::Serde(Value::String(endpoint.to_string()))],
    ));

    match result {
        Ok(_) => Ok(Ok(())),
        Err(e) => match e {
            DenoError::Explicit(e) => Ok(Err(anyhow!(e))),
            e => Err(anyhow!(e)),
        },
    }
}
