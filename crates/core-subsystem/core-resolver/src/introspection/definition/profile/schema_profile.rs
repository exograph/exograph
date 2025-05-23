// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::operation_set::{OperationSet, OperationSetSerialized};
use core_model::primitive_type::PrimitiveType;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemaProfileSerialized {
    queries: Option<OperationSetSerialized>,
    mutations: Option<OperationSetSerialized>,
}

#[derive(Debug)]
pub struct SchemaProfile {
    pub(super) queries: OperationSet,
    pub(super) mutations: OperationSet,
}

impl SchemaProfile {
    pub fn all() -> Self {
        Self {
            queries: OperationSet::all(),
            mutations: OperationSet::all(),
        }
    }

    pub fn queries_only() -> Self {
        Self {
            queries: OperationSet::all(),
            mutations: OperationSet::none(),
        }
    }

    pub fn query_matches(&self, model_name: &str, operation_name: &str) -> bool {
        (PrimitiveType::is_primitive(model_name) || self.queries.models.matches(model_name))
            && self.queries.operations.matches(operation_name)
    }

    pub fn mutation_matches(&self, model_name: &str, operation_name: &str) -> bool {
        (PrimitiveType::is_primitive(model_name) || self.mutations.models.matches(model_name))
            && self.mutations.operations.matches(operation_name)
    }
}

impl From<SchemaProfileSerialized> for SchemaProfile {
    fn from(serialized: SchemaProfileSerialized) -> Self {
        SchemaProfile {
            queries: serialized.queries.map_or(OperationSet::all(), |q| q.into()),
            mutations: serialized
                .mutations
                .map_or(OperationSet::none(), |m| m.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_specified() {
        let serialized = r#"
        {
            "queries": {
                "models": {
                    "include": ["User", "Post"],
                    "exclude": ["Comment"]
                },
                "operations": {
                    "include": ["users", "posts"],
                    "exclude": ["comments"]
                }
            },
            "mutations": {
                "models": {
                    "include": ["User", "Post"],
                    "exclude": ["Comment"]
                },
                "operations": {
                    "include": ["createUser", "updateUser"],
                    "exclude": ["deleteUser"]
                }
            }
        }"#;

        let profile = serde_json::from_str::<SchemaProfileSerialized>(serialized).unwrap();
        let profile: SchemaProfile = profile.into();

        assert!(profile.query_matches("User", "users"));
        assert!(profile.query_matches("Post", "posts"));
        assert!(!profile.query_matches("Comment", "comments"));
        assert!(!profile.query_matches("User", "comments"));

        assert!(profile.mutation_matches("User", "createUser"));
        assert!(profile.mutation_matches("User", "updateUser"));
        assert!(!profile.mutation_matches("User", "deleteUser"));
    }

    #[test]
    fn nothing_specified() {
        let serialized = r#"{}"#;
        let profile = serde_json::from_str::<SchemaProfileSerialized>(serialized).unwrap();
        let profile: SchemaProfile = profile.into();

        // By default, we allow all queries, but no mutations
        assert!(profile.query_matches("User", "users"));
        assert!(!profile.mutation_matches("User", "createUser"));
    }

    #[test]
    fn models_operations_include_all() {
        let serialized = r#"
        {
            "queries": {
                "models": {
                    "include": ["User", "Post"],
                    "exclude": ["Comment"]
                },
                "operations": {
                    "include": ["*"]
                }
            }
        }"#;

        let profile = serde_json::from_str::<SchemaProfileSerialized>(serialized).unwrap();
        let profile: SchemaProfile = profile.into();

        assert!(profile.query_matches("User", "users"));
        assert!(profile.query_matches("Post", "posts"));
        assert!(!profile.query_matches("Comment", "comments"));

        assert!(!profile.mutation_matches("User", "createUser"));
        assert!(!profile.mutation_matches("User", "updateUser"));
        assert!(!profile.mutation_matches("User", "deleteUser"));
    }

    #[test]
    fn models_operations_exclude_all() {
        let serialized = r#"
        {
            "queries": {
                "models": {
                    "include": ["User", "Post"],
                    "exclude": ["Comment"]
                },
                "operations": {
                    "exclude": ["*"]
                }
            }
        }"#;

        let profile = serde_json::from_str::<SchemaProfileSerialized>(serialized).unwrap();
        let profile: SchemaProfile = profile.into();

        assert!(!profile.query_matches("User", "users"));
        assert!(!profile.query_matches("Post", "posts"));
        assert!(!profile.query_matches("Comment", "comments"));

        assert!(!profile.mutation_matches("User", "createUser"));
        assert!(!profile.mutation_matches("User", "updateUser"));
        assert!(!profile.mutation_matches("User", "deleteUser"));
    }
}
