// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::Environment;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct MapEnvironment {
    values: HashMap<String, String>,
    fallback: Option<Arc<dyn Environment>>,
}

impl Environment for MapEnvironment {
    fn get(&self, key: &str) -> Option<String> {
        self.values
            .get(key)
            .cloned()
            .or_else(|| self.fallback.as_ref().and_then(|fb| fb.get(key)))
    }
}

impl From<HashMap<String, String>> for MapEnvironment {
    fn from(values: HashMap<String, String>) -> Self {
        Self {
            values,
            fallback: None,
        }
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
            fallback: None,
        }
    }
}

impl MapEnvironment {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            fallback: None,
        }
    }

    pub fn new_with_fallback(fallback: Arc<dyn Environment>) -> Self {
        Self {
            values: HashMap::new(),
            fallback: Some(fallback),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_string(), value.to_string());
    }

    pub fn vars(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.values.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}
