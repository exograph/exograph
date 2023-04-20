// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use async_graphql_parser::{
    types::{DocumentOperations, ExecutableDocument, FragmentDefinition, Selection, SelectionSet},
    Positioned,
};
use async_graphql_value::{Name, Value};

pub type TestvariableBindings = HashMap<String, TestvariablePath>;
type TestvariablePath = Vec<TestvariablePathElements>;
type TestvariablePathElements = String;

// Generate test variable bindings from a GraphQL document
//
// Test variable bindings consist of a hashmap mapping variable names to 'paths' for JSON objects:
//
// let bindings: HashMap<_,_> = vec![
//    ("x", vec!["foo", "bar", "qux") ],
//    ("y": vec!["foo", "bar", "biz") ]
// ]
//    .into_iter()
//    .collect();
//
// Using this mapping, we can later resolve values using a variable name and a binding map.
// For example, with this response and the above bindings:
//
// { "data":
//    {
//        "foo": {
//            "bar": {
//                "qux": 1,
//                "biz": [ "a", "b", "c"]
//            }
//        }
//    }
// }
//
// ... we can resolve variable `x` to 1 and variable `y` to ["a", "b", "c"].
//
// This function generates variable bindings from GraphQL fields marked with the @bind directive.
// See unit tests for usage.
pub fn build_testvariable_bindings(document: &ExecutableDocument) -> TestvariableBindings {
    match &document.operations {
        DocumentOperations::Single(operation) => {
            let selection_set = &operation.node.selection_set.node;
            process_selection_set(
                selection_set,
                vec!["data".to_owned()],
                &document.fragments,
                HashSet::new(),
            )
        }
        DocumentOperations::Multiple(operations) => operations
            .iter()
            .flat_map(|(name, operation)| {
                let selection_set = &operation.node.selection_set.node;
                process_selection_set(
                    selection_set,
                    vec!["data".to_owned(), name.to_string()],
                    &document.fragments,
                    HashSet::new(),
                )
            })
            .collect(),
    }
}

fn process_selection_set(
    selection_set: &SelectionSet,
    current_path: Vec<TestvariablePathElements>,
    fragments: &HashMap<Name, Positioned<FragmentDefinition>>,
    fragment_trail: HashSet<String>,
) -> TestvariableBindings {
    selection_set
        .items
        .iter()
        .map(|p| &p.node)
        .flat_map(|selection| {
            match selection {
                Selection::Field(field) => {
                    let field = &field.node;
                    let field_name = field.alias.as_ref().unwrap_or(&field.name).to_string();
                    let selection_set = &field.selection_set.node;

                    // we found a field, append name to path
                    let mut new_path = current_path.clone();
                    new_path.push(field_name);

                    let mut bindings = HashMap::new();

                    if let Some(bind_directive) = field
                        .directives
                        .iter()
                        .find(|p| p.node.name.node.as_str() == "bind")
                    {
                        // binding found
                        let binding_name = bind_directive
                            .node
                            .arguments
                            .iter()
                            .find_map(|(arg_name, arg)| {
                                if arg_name.node.as_str() == "name" {
                                    match &arg.node {
                                        Value::String(s) => Some(s),
                                        _ => panic!("name provided to @bind is not a string"),
                                    }
                                } else {
                                    None
                                }
                            })
                            .expect("No name provided for @bind directive");

                        bindings.insert(binding_name.clone(), new_path.clone());
                    }

                    // continue building bindings for our selection tree
                    bindings.extend(process_selection_set(
                        selection_set,
                        new_path.clone(),
                        fragments,
                        fragment_trail.clone(),
                    ));

                    bindings
                }

                Selection::FragmentSpread(fragment_spread) => {
                    let fragment_spread = &fragment_spread.node;
                    let fragment_name = &fragment_spread.fragment_name.node;
                    let selection_set = if let Some(fragment) = fragments.get(fragment_name) {
                        &fragment.node.selection_set.node
                    } else {
                        // soft fail - some tests may actually depend on missing fragments
                        return Default::default();
                    };

                    if fragment_trail.contains(fragment_name.as_str()) {
                        // soft fail - some tests may actually depend on circular fragments (will fail at runtime)
                        return Default::default();
                    }
                    let fragment_trail = {
                        let mut fragment_trail = fragment_trail.clone();
                        fragment_trail.insert(fragment_name.to_string());
                        fragment_trail
                    };

                    process_selection_set(
                        selection_set,
                        current_path.clone(),
                        fragments,
                        fragment_trail,
                    )
                }
                Selection::InlineFragment(inline_fragment) => {
                    let selection_set = &inline_fragment.node.selection_set.node;

                    process_selection_set(
                        selection_set,
                        current_path.clone(),
                        fragments,
                        fragment_trail.clone(),
                    )
                }
            }
        })
        .collect::<HashMap<_, _>>()
}

// Resolve the value of a test variable from `response` using its name and the set of variable bindings.
pub fn resolve_testvariable(
    variable_name: &str,
    response: &serde_json::Value,
    bindings: &TestvariableBindings,
) -> Result<serde_json::Value> {
    let starting_path = bindings
        .get(variable_name)
        .ok_or_else(|| anyhow!("variable {} does not exist", variable_name))?;

    fn recursive_resolve(
        path: &[String],
        base_value: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        match path {
            [] => Ok(base_value.clone()),
            [key, path_tail @ ..] => {
                match base_value {
                    // binding on a pk query (e.g. log(id: 2) {
                    //    id @(name: "log_id")
                    // })
                    serde_json::Value::Object(obj) => {
                        if let Some(value) = obj.get(key) {
                            recursive_resolve(path_tail, value)
                        } else {
                            Err(anyhow!("key {} does not exist in {:#?}", key, obj))
                        }
                    }

                    // binding on a field in a collection query (e.g. logs() {
                    //      id @bind(name: "logs_id")
                    // })
                    //
                    // so we'd see something like
                    // logs: [ { "id": 1 }, { "id": 2 }, ... ]
                    //
                    // recursively resolve each element using the same path as before
                    serde_json::Value::Array(vec) => {
                        let resolved = vec
                            .iter()
                            .map(|value| recursive_resolve(path, value))
                            .collect::<Result<Vec<_>>>()?;

                        Ok(serde_json::Value::Array(resolved))
                    }
                    _ => Err(anyhow!("cannot index into {}", base_value)),
                }
            }
        }
    }

    recursive_resolve(starting_path, response)
}

#[cfg(test)]
mod tests {
    use super::{build_testvariable_bindings, resolve_testvariable};

    const GQL: &str = r#"
        query {
            createLog(data: {}) {
                id @bind(name: "createLog_id")
            }

            log1: createLogs(data: []) {
                id @bind(name: "log1_ids")
            }
        }    
    "#;

    #[test]
    fn test_bindings_build() {
        let document = async_graphql_parser::parse_query(GQL).unwrap();
        let bindings = build_testvariable_bindings(&document);

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(bindings);
        });
    }

    #[test]
    fn test_resolution() {
        let document = async_graphql_parser::parse_query(GQL).unwrap();
        let bindings = build_testvariable_bindings(&document);
        let response = serde_json::from_str(
            r#"
            {
                "data": {
                    "createLog": {
                        "id": 1
                    },

                    "log1": [
                        { "id": 2 },
                        { "id": 3 },
                        { "id": 4 }
                    ]
                }
            }
        "#,
        )
        .unwrap();

        let create_log_id = resolve_testvariable("createLog_id", &response, &bindings).unwrap();
        let log1_ids = resolve_testvariable("log1_ids", &response, &bindings).unwrap();

        println!("{log1_ids:#?}");

        assert_eq!(create_log_id, 1);
        assert_eq!(log1_ids, serde_json::to_value(vec![2, 3, 4]).unwrap());
    }
}
