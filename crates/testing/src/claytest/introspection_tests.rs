use anyhow::{anyhow, bail, Result};
use isahc::{HttpClient, ReadResponseExt, Request};
use serde_json::{json, Value};
use std::{collections::HashSet, path::Path};

use crate::claytest::common::{spawn_clay_server, TestResultKind};

use super::common::TestResult;

const INTROSPECTION_QUERY: &str = include_str!("introspection-query.gql");

pub(crate) fn run_introspection_test(model_path: &Path) -> Result<TestResult> {
    let log_prefix =
        ansi_term::Color::Purple.paint(format!("(introspection: {})\n :: ", model_path.display()));
    println!("{} Running introspection tests...", log_prefix);

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
            let result: Value = result.json()?;
            let output: String = server.output.lock().unwrap().clone();

            Ok(match check_introspection(result) {
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

fn check_introspection(introspection: Value) -> Result<()> {
    check_query_mutation_duplication(&introspection)?;

    Ok(())
}

fn check_query_mutation_duplication(introspection: &serde_json::Value) -> Result<()> {
    let (query, mutation) = get_query_and_mutation_type(introspection)?;

    fn check_duplication(obj: &Object) -> Result<()> {
        let obj_name = obj.get("name").unwrap().as_str().unwrap();

        let fields = obj
            .get("fields")
            .ok_or_else(|| anyhow!("No entry named fields in {}", obj_name))?;

        let fields = fields
            .as_array()
            .ok_or_else(|| anyhow!("fields is not an array"))?;

        let mut set: HashSet<&str> = HashSet::new();
        for field in fields {
            let name = field
                .get("name")
                .ok_or_else(|| anyhow!("No name field for method: {}", field.to_string()))?
                .as_str()
                .ok_or_else(|| anyhow!("Method name is not a string: {}", field.to_string()))?;

            if !set.insert(name) {
                bail!("Method {} is duplicated in {}", name, obj_name)
            }
        }

        Ok(())
    }

    check_duplication(query)?;
    check_duplication(mutation)?;

    Ok(())
}

type Object = serde_json::Map<String, Value>;

fn get_schema(introspection: &Value) -> Result<&Value> {
    let schema = introspection
        .get("data")
        .ok_or_else(|| anyhow!("No data field in introspection"))?
        .get("__schema")
        .ok_or_else(|| anyhow!("No __schema field in response data"))?;

    Ok(schema)
}

fn get_type<'a>(introspection: &'a Value, name: &'a str) -> Result<Option<&Object>> {
    let schema = get_schema(introspection)?;
    let types = schema
        .get("types")
        .ok_or_else(|| anyhow!("No types in schema"))?
        .as_array()
        .ok_or_else(|| anyhow!("types field in schema is not an array"))?;

    for typ in types {
        if let Value::Object(obj) = typ {
            let typ_name = obj
                .get("name")
                .ok_or_else(|| anyhow!("No name field in type"))?
                .as_str()
                .ok_or_else(|| anyhow!("name field in type is not string"))?;

            if typ_name == name {
                return Ok(Some(obj));
            }
        } else {
            bail!("Type is not an object: {}", typ.to_string())
        }
    }

    Ok(None)
}

fn get_query_and_mutation_type(introspection: &Value) -> Result<(&Object, &Object)> {
    let schema = get_schema(introspection)?;
    let query_type = schema
        .get("queryType")
        .ok_or_else(|| anyhow!("No queryType field in schema"))?
        .get("name")
        .ok_or_else(|| anyhow!("queryType is missing name"))?
        .as_str()
        .ok_or_else(|| anyhow!("queryType.name is not a string"))?;

    let mutation_type = schema
        .get("mutationType")
        .ok_or_else(|| anyhow!("No mutationType field in schema"))?
        .get("name")
        .ok_or_else(|| anyhow!("mutationType is missing name"))?
        .as_str()
        .ok_or_else(|| anyhow!("mutationType.name is not a string"))?;

    let query =
        get_type(introspection, query_type)?.ok_or_else(|| anyhow!("No query type exists"))?;
    let mutation = get_type(introspection, mutation_type)?
        .ok_or_else(|| anyhow!("No mutation type exists"))?;

    Ok((query, mutation))
}
