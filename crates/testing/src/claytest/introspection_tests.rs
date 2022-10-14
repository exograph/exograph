use anyhow::{bail, Result};
use include_dir::{include_dir, Dir};
use isahc::{HttpClient, ReadResponseExt, Request};
use payas_deno::{DenoModule, DenoModuleSharedState, UserCode};
use serde_json::{json, Value};
use std::{collections::HashMap, path::Path};

use crate::claytest::{
    common::{spawn_clay_server, TestResultKind},
    integration_tests::build_claypot_file,
};

use super::common::TestResult;

const INTROSPECTION_QUERY: &str = include_str!("introspection-query.gql");
const INTROSPECTION_ASSERT_JS: &str = include_str!("introspection_tests.js");
const GRAPHQL_NODE_MODULE: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/../../graphiql/node_modules/graphql");

pub(crate) fn run_introspection_test(model_path: &Path) -> Result<TestResult> {
    let log_prefix =
        ansi_term::Color::Purple.paint(format!("(introspection: {})\n :: ", model_path.display()));
    println!("{} Running introspection tests...", log_prefix);

    build_claypot_file(model_path)?;

    let server = spawn_clay_server(
        model_path,
        [
            ("CLAY_INTROSPECTION", "true"),
            ("CLAY_DATABASE_URL", "postgres://a@dummy-value"),
            ("CLAY_CHECK_CONNECTION_ON_STARTUP", "false"),
            ("CLAY_SERVER_PORT", "0"), // ask clay-server to select a free port
        ]
        .into_iter(),
    )?;

    // spawn an HttpClient for requests to clay
    let client = HttpClient::builder().build()?;

    let response = client.send(
        Request::post(&server.endpoint)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&json!({
                "query": INTROSPECTION_QUERY
            }))?)?,
    );

    match response {
        Ok(mut result) => {
            let response: Value = result.json()?;
            let output: String = server.output.lock().unwrap().clone();

            Ok(match check_introspection(response) {
                Ok(()) => TestResult {
                    log_prefix: log_prefix.to_string(),
                    result: TestResultKind::Success,
                    output,
                },

                Err(e) => TestResult {
                    log_prefix: log_prefix.to_string(),
                    result: TestResultKind::Fail(e),
                    output,
                },
            })
        }
        Err(e) => {
            bail!("Error while making request: {}", e)
        }
    }
}

fn check_introspection(response: Value) -> Result<()> {
    let response = response.to_string();
    let script = INTROSPECTION_ASSERT_JS;
    let script = script.replace("\"%%RESPONSE%%\"", &response);

    let deno_module_future = DenoModule::new(
        UserCode::LoadFromMemory {
            path: "internal/introspection_tests.js".to_owned(),
            script: script.into(),
        },
        "ClaytipTest",
        vec![],
        vec![],
        vec![],
        DenoModuleSharedState::default(),
        None,
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
    match runtime.block_on(deno_module_future) {
        Ok(_) => Ok(()),
        Err(e) => bail!("{}", e),
    }
}
