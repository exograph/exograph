use std::collections::HashMap;

use anyhow::{anyhow, Result};
use payas_deno::{Arg, DenoModule, DenoModuleSharedState, UserCode};

const ASSERT_JS: &str = include_str!("./assert.js");

// Assert that a stringified 'JSON' payload is equal to a JSON object.
//
// `expected` is a string and not a proper JSON value because it is meant to be evaluated
// in a JavaScript environment (in this case, using payas-deno). This means that we can specify
// dynamic assertions in .claytest files in the form of JavaScript code, as well as bind values from
// claytest init-*.gql files into our expected payload dynamically.
//
// `expected` should be a valid stringified JSON payload except for the following differences:
//
// - values may access bound values (using @bind in init files on selection fields) using the `$` object
//     - bound values must be provided to the function through the `testvariables` arg
// - values may perform their own assertions by specifying a JavaScript closure that
//   takes the actual value and returns a Boolean (or throws a ClaytipError)
//
// See unit tests for usage.
pub fn dynamic_assert_using_deno(
    expected: &str,
    actual: serde_json::Value,
    prelude: &str,
    testvariables: &HashMap<String, serde_json::Value>,
) -> Result<()> {
    let testvariables_json = serde_json::to_value(testvariables)?;

    // first substitute expected variables
    let script = ASSERT_JS.to_owned();
    let script = script.replace("\"%%PRELUDE%%\"", prelude);
    let script = script.replace("\"%%JSON%%\"", expected);

    let deno_module_future = DenoModule::new(
        UserCode::LoadFromMemory {
            path: "internal/assert.js".to_owned(),
            script: script.into(),
        },
        "ClaytipTest",
        vec![],
        vec![include_str!(
            "../../../../deno-subsystem/deno-resolver/src/claytip_error.js"
        )],
        vec![],
        DenoModuleSharedState::default(),
        Some("ClaytipError"),
        None,
        None,
    );

    let runtime = tokio::runtime::Runtime::new()?;
    let mut deno_module = runtime.block_on(deno_module_future)?;

    // run method
    let _ = runtime
        .block_on(deno_module.execute_function(
            "test",
            vec![Arg::Serde(actual.clone()), Arg::Serde(testvariables_json)],
        ))
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
pub fn evaluate_using_deno(
    not_really_json: &str,
    prelude: &str,
    testvariables: &HashMap<String, serde_json::Value>,
) -> Result<serde_json::Value> {
    let testvariables_json = serde_json::to_value(testvariables)?;

    // first substitute expected variables
    let script = ASSERT_JS.to_owned();
    let script = script.replace("\"%%PRELUDE%%\"", prelude);
    let script = script.replace("\"%%JSON%%\"", not_really_json);

    let deno_module_future = DenoModule::new(
        UserCode::LoadFromMemory {
            path: "internal/assert.js".to_owned(),
            script: script.into(),
        },
        "ClaytipTest",
        vec![],
        vec![],
        vec![],
        DenoModuleSharedState::default(),
        None,
        None,
        None,
    );

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut deno_module = runtime.block_on(deno_module_future)?;

    // run method
    runtime.block_on(async {
        deno_module
            .execute_function("evaluate", vec![Arg::Serde(testvariables_json)])
            .await
            .map_err(|e| anyhow!(e))
    })
}

#[cfg(test)]
mod tests {
    use crate::claytest::assertion::evaluate_using_deno;

    use super::dynamic_assert_using_deno;

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

    #[test]
    fn test_dynamic_assert() {
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

        dynamic_assert_using_deno(expected, actual_payload(), "", &testvariables).unwrap();
    }

    #[test]
    fn test_evaluation() {
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

        let result = evaluate_using_deno(payload, "", &testvariables).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(result);
        });
    }

    #[test]
    fn test_dynamic_assert_failing_normal_payloads() {
        let expected = r#"
            {
                "data": {
                    "a": 1, 
                    "b": ["foo", "bar"],
                    "c": "biz" 
                }
            }
        "#;

        let testvariables = vec![].into_iter().collect();

        let err =
            dynamic_assert_using_deno(expected, actual_payload(), "", &testvariables).unwrap_err();

        assert!(err
            .to_string()
            .starts_with("assert failed: expected biz on key c, got qux"));
    }

    #[test]
    fn test_dynamic_assert_failing_closure_test() {
        let expected = r#"
            {
                "data": {
                    "a": 1, 
                    "b": ["foo", "bar"],
                    "c": ((actual) => { return false; }) 
                }
            }
        "#;

        let testvariables = vec![].into_iter().collect();

        let err =
            dynamic_assert_using_deno(expected, actual_payload(), "", &testvariables).unwrap_err();

        assert!(err
            .to_string()
            .starts_with("assert function failed for field c!"));
    }

    #[test]
    fn test_deno_prelude_and_async() {
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

        let testvariables = vec![].into_iter().collect();
        dynamic_assert_using_deno(expected, actual_payload(), prelude, &testvariables).unwrap();
    }
}
