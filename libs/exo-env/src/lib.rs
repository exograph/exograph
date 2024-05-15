// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

pub trait Environment: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
}

pub struct SystemEnvironment;

impl Environment for SystemEnvironment {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

pub struct MapEnvironment {
    values: HashMap<String, String>,
}

impl Environment for MapEnvironment {
    fn get(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }
}

impl From<HashMap<String, String>> for MapEnvironment {
    fn from(values: HashMap<String, String>) -> Self {
        Self { values }
    }
}

impl<const N: usize> From<[(&str, &str); N]> for MapEnvironment {
    fn from(values: [(&str, &str); N]) -> Self {
        Self {
            values: HashMap::from_iter(
                values
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string())),
            ),
        }
    }
}
