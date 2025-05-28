// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::operation_set::OperationSet;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct SchemaProfile {
    pub queries: OperationSet,
    pub mutations: OperationSet,
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

    pub fn query_matches(
        &self,
        model_name: &str,
        operation_name: &str,
        is_primitive: impl Fn(&str) -> bool,
    ) -> bool {
        (is_primitive(model_name) || self.queries.models.matches(model_name))
            && self.queries.operations.matches(operation_name)
    }

    pub fn mutation_matches(
        &self,
        model_name: &str,
        operation_name: &str,
        is_primitive: impl Fn(&str) -> bool,
    ) -> bool {
        (is_primitive(model_name) || self.mutations.models.matches(model_name))
            && self.mutations.operations.matches(operation_name)
    }
}

#[cfg(test)]
mod tests {
    use wildmatch::WildMatch;

    use crate::profile::InclusionExclusion;

    use super::*;

    #[test]
    fn everything_specified() {
        let profile: SchemaProfile = SchemaProfile {
            queries: OperationSet {
                models: InclusionExclusion {
                    include: vec![WildMatch::new("User"), WildMatch::new("Post")],
                    exclude: vec![WildMatch::new("Comment")],
                },
                operations: InclusionExclusion {
                    include: vec![WildMatch::new("users"), WildMatch::new("posts")],
                    exclude: vec![WildMatch::new("comments")],
                },
            },
            mutations: OperationSet {
                models: InclusionExclusion {
                    include: vec![WildMatch::new("User"), WildMatch::new("Post")],
                    exclude: vec![WildMatch::new("Comment")],
                },
                operations: InclusionExclusion {
                    include: vec![WildMatch::new("createUser"), WildMatch::new("updateUser")],
                    exclude: vec![WildMatch::new("deleteUser")],
                },
            },
        };

        assert!(profile.query_matches("User", "users", |_| false));
        assert!(profile.query_matches("Post", "posts", |_| false));
        assert!(!profile.query_matches("Comment", "comments", |_| false));
        assert!(!profile.query_matches("User", "comments", |_| false));

        assert!(profile.mutation_matches("User", "createUser", |_| false));
        assert!(profile.mutation_matches("User", "updateUser", |_| false));
        assert!(!profile.mutation_matches("User", "deleteUser", |_| false));
    }

    #[test]
    fn queries_only() {
        let profile: SchemaProfile = SchemaProfile::queries_only();

        assert!(profile.query_matches("User", "users", |_| false));
        assert!(!profile.mutation_matches("User", "createUser", |_| false));
    }

    #[test]
    fn models_operations_include_all() {
        let profile: SchemaProfile = SchemaProfile {
            queries: OperationSet {
                models: InclusionExclusion {
                    include: vec![WildMatch::new("User"), WildMatch::new("Post")],
                    exclude: vec![WildMatch::new("Comment")],
                },
                operations: InclusionExclusion {
                    include: vec![WildMatch::new("*")],
                    exclude: vec![],
                },
            },
            mutations: OperationSet::none(),
        };

        assert!(profile.query_matches("User", "users", |_| false));
        assert!(profile.query_matches("Post", "posts", |_| false));
        assert!(!profile.query_matches("Comment", "comments", |_| false));

        assert!(!profile.mutation_matches("User", "createUser", |_| false));
        assert!(!profile.mutation_matches("User", "updateUser", |_| false));
        assert!(!profile.mutation_matches("User", "deleteUser", |_| false));
    }

    #[test]
    fn models_operations_exclude_all() {
        let profile: SchemaProfile = SchemaProfile {
            queries: OperationSet {
                models: InclusionExclusion {
                    include: vec![WildMatch::new("User"), WildMatch::new("Post")],
                    exclude: vec![WildMatch::new("Comment")],
                },
                operations: InclusionExclusion {
                    include: vec![],
                    exclude: vec![WildMatch::new("*")],
                },
            },
            mutations: OperationSet::none(),
        };

        assert!(!profile.query_matches("User", "users", |_| false));
        assert!(!profile.query_matches("Post", "posts", |_| false));
        assert!(!profile.query_matches("Comment", "comments", |_| false));

        assert!(!profile.mutation_matches("User", "createUser", |_| false));
        assert!(!profile.mutation_matches("User", "updateUser", |_| false));
        assert!(!profile.mutation_matches("User", "deleteUser", |_| false));
    }
}
