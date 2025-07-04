// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow};
use exo_deno::{
    Arg, DenoModule, DenoModuleSharedState, UserCode,
    deno_core::{ModuleType, url::Url},
    deno_executor_pool::{DenoScriptDefn, ResolvedModule},
};

const ASSERT_JS: &str = include_str!("./assert.js");

// Assert that a stringified 'JSON' payload is equal to a JSON object.
//
// `expected` is a string and not a proper JSON value because it is meant to be evaluated
// in a JavaScript environment (in this case, using exo-deno). This means that we can specify
// dynamic assertions in .exotest files in the form of JavaScript code, as well as bind values from
// exotest init-*.gql files into our expected payload dynamically.
//
// `expected` should be a valid stringified JSON payload except for the following differences:
//
// - values may access bound values (using @bind in init files on selection fields) using the `$` object
//     - bound values must be provided to the function through the `testvariables` arg
// - values may perform their own assertions by specifying a JavaScript closure that
//   takes the actual value and returns a Boolean (or throws a ExographError)
//
// See unit tests for usage.
pub async fn dynamic_assert_using_deno(
    expected: &str,
    actual: serde_json::Value,
    prelude: &str,
    testvariables: &HashMap<String, serde_json::Value>,
    unordeded_selections: &HashSet<Vec<String>>,
) -> Result<()> {
    let testvariables_json = serde_json::to_value(testvariables)?;
    let unordeded_selections_json = serde_json::to_value(unordeded_selections)?;

    // first substitute expected variables
    let script = ASSERT_JS.to_owned();
    let script = script.replace("\"%%PRELUDE%%\"", prelude);
    let script = script.replace("\"%%JSON%%\"", expected);

    let mut deno_module = create_deno_module(script).await?;

    // run method
    let _ = deno_module
        .execute_function(
            "dynamic_assert",
            vec![
                Arg::Serde(actual.clone()),
                Arg::Serde(testvariables_json),
                Arg::Serde(unordeded_selections_json),
            ],
        )
        .await
        .map_err(|e| {
            anyhow!(
                "{}\n➞ Expected: \n{}\n➞ Got: \n{}\n",
                e,
                expected,
                serde_json::to_string_pretty(&actual).unwrap()
            )
        })?;

    Ok(())
}

// Evaluates substitutions only in a stringified 'JSON' payload.
//
// Used to substitute variable in `variable`, `headers`, and `auth` from gql/exotest files.
pub async fn evaluate_using_deno(
    not_really_json: &str,
    prelude: &str,
    testvariables: &HashMap<String, serde_json::Value>,
) -> Result<serde_json::Value> {
    let testvariables_json = serde_json::to_value(testvariables)?;

    // first substitute expected variables
    let script = ASSERT_JS.to_owned();
    let script = script.replace("\"%%PRELUDE%%\"", prelude);
    let script = script.replace("\"%%JSON%%\"", not_really_json);

    let mut deno_module = create_deno_module(script).await?;

    // run method
    deno_module
        .execute_function("evaluate", vec![Arg::Serde(testvariables_json)])
        .await
        .map_err(|e| anyhow!(e))
}

// Evaluates substitutions only in a stringified 'JSON' payload.
//
// Used to substitute variable in `variable`, `headers`, and `auth` from gql/exotest files.
pub async fn assert_using_deno(
    actual: serde_json::Value,
    expected: serde_json::Value,
    unordeded_selections: &HashSet<Vec<String>>,
) -> Result<()> {
    let script = ASSERT_JS.to_owned();
    let unordeded_selections_json = serde_json::to_value(unordeded_selections)?;

    let mut deno_module = create_deno_module(script).await?;

    // run method
    deno_module
        .execute_function(
            "assert",
            vec![
                Arg::Serde(expected.clone()),
                Arg::Serde(actual.clone()),
                Arg::Serde(unordeded_selections_json),
            ],
        )
        .await
        .map_err(|e| {
            anyhow!(
                "{}\n➞ Expected: \n{}\n➞ Got: \n{}\n",
                e,
                serde_json::to_string_pretty(&expected).unwrap(),
                serde_json::to_string_pretty(&actual).unwrap()
            )
        })?;

    Ok(())
}

async fn create_deno_module(script: String) -> Result<DenoModule> {
    let deno_module = DenoModule::new(
        UserCode::LoadFromMemory {
            path: "file:///internal/assert.js".to_owned(),
            script: DenoScriptDefn {
                modules: vec![(
                    Url::parse("file:///internal/assert.js").unwrap(),
                    ResolvedModule::Module(
                        script,
                        ModuleType::JavaScript,
                        Url::parse("file:///internal/assert.js").unwrap(),
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
        vec![include_str!(
            "../../../../deno-subsystem/deno-graphql-resolver/src/exograph_error.js"
        )],
        vec![],
        DenoModuleSharedState::default(),
        Some("ExographError"),
        None,
        None,
    )
    .await?;

    Ok(deno_module)
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;

    fn actual_payload() -> serde_json::Value {
        serde_json::from_str(
            r#"
            {
                "data": {
                    "a": 1,
                    "b": ["foo", "bar"],
                    "c": "qux"
                }
            }
        "#,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn dynamic_assert() {
        let expected = r#"
            {
                "data": {
                    "a": 1,
                    "b": $.b,
                    "c": RegExp.prototype.test.bind(/q.*/g)
                }
            }
        "#;

        let testvariables = vec![(
            "b".to_owned(),
            serde_json::to_value(vec!["foo", "bar"]).unwrap(),
        )]
        .into_iter()
        .collect();

        dynamic_assert_using_deno(
            expected,
            actual_payload(),
            "",
            &testvariables,
            &HashSet::new(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn evaluation() {
        let payload = r#"
            {
                "data": {
                    "a": 1,
                    "b": $.b,
                    "c": function () { return "this function should disappear since it is not JSON"; }
                }
            }
        "#;

        let testvariables = vec![(
            "b".to_owned(),
            serde_json::to_value(vec!["foo", "bar"]).unwrap(),
        )]
        .into_iter()
        .collect();

        let result = evaluate_using_deno(payload, "", &testvariables)
            .await
            .unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(result);
        });
    }

    #[tokio::test]
    async fn dynamic_assert_failing_normal_payloads() {
        let expected = r#"
            {
                "data": {
                    "a": 1,
                    "b": ["foo", "bar"],
                    "c": "biz"
                }
            }
        "#;

        let testvariables = HashMap::new();

        let err = dynamic_assert_using_deno(
            expected,
            actual_payload(),
            "",
            &testvariables,
            &HashSet::new(),
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string()
                .starts_with("assertion failed at 'data.c': expected biz, got qux")
        );
    }

    #[tokio::test]
    async fn dynamic_assert_failing_closure_test() {
        let expected = r#"
            {
                "data": {
                    "a": 1,
                    "b": ["foo", "bar"],
                    "c": ((actual) => { return false; })
                }
            }
        "#;

        let testvariables = HashMap::new();

        let err = dynamic_assert_using_deno(
            expected,
            actual_payload(),
            "",
            &testvariables,
            &HashSet::new(),
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string()
                .starts_with("assertion failed at 'data.c': assert function failed actual")
        );
    }

    #[tokio::test]
    async fn deno_prelude_and_async() {
        let prelude = r#"
            function someAsyncOp() {
                return new Promise(resolve => setTimeout(resolve, 1000));
            }
        "#;

        let expected = r#"
            {
                "data": {
                    "a": 1,
                    "b": ["foo", "bar"],
                    "c": async function(actual) { return await someAsyncOp(); }
                }
            }
        "#;

        let testvariables = HashMap::new();

        dynamic_assert_using_deno(
            expected,
            actual_payload(),
            prelude,
            &testvariables,
            &HashSet::new(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn unordered_dynamic_and_static_assert() {
        let expected = r#"
            {
                "data": {
                    "products": [
                        {
                            "id": 1,
                            "name": "foo"
                        },
                        {
                            "id": 2,
                            "name": "bar"
                        },
                        {
                            "id": 3,
                            "name": "baz"
                        }
                    ]
                }
            }
        "#;

        let actual = r#"
            {
                "data": {
                    "products": [
                        {
                            "id": 2,
                            "name": "bar"
                        },
                        {
                            "id": 3,
                            "name": "baz"
                        },
                        {
                            "id": 1,
                            "name": "foo"
                        }
                    ]
                }
            }
        "#;

        let testvariables = HashMap::new();

        // success case
        dynamic_assert_using_deno(
            expected,
            serde_json::from_str(actual).unwrap(),
            "",
            &testvariables,
            &vec![vec!["data".to_string(), "products".to_string()]]
                .into_iter()
                .collect(),
        )
        .await
        .unwrap();

        assert_using_deno(
            serde_json::from_str(expected).unwrap(),
            serde_json::from_str(actual).unwrap(),
            &vec![vec!["data".to_string(), "products".to_string()]]
                .into_iter()
                .collect(),
        )
        .await
        .unwrap();

        // failure cases (non-matching selections)
        for unordered_selection in [
            vec![].into_iter().collect(),
            vec!["data".to_string()].into_iter().collect(),
            vec!["products".to_string()].into_iter().collect(),
            vec!["id".to_string()].into_iter().collect(),
        ] {
            let unordered_selection = vec![unordered_selection].into_iter().collect();

            let result = dynamic_assert_using_deno(
                expected,
                serde_json::from_str(actual).unwrap(),
                "",
                &testvariables,
                &unordered_selection,
            )
            .await;
            assert!(result.is_err(), "Dynamic assert should fail");

            let result = assert_using_deno(
                serde_json::from_str(expected).unwrap(),
                serde_json::from_str(actual).unwrap(),
                &unordered_selection,
            )
            .await;

            assert!(result.is_err(), "Assert should fail");
        }
    }
}
