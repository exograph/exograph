// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::Environment;
use std::sync::Arc;

pub struct CompositeEnvironment {
    // Underlying environments in order of precedence (first is highest precedence)
    envs: Vec<Arc<dyn Environment>>,
}

impl Environment for CompositeEnvironment {
    fn get(&self, key: &str) -> Option<String> {
        self.envs.iter().find_map(|e| e.get(key))
    }

    fn non_system_envs(&self) -> Box<dyn Iterator<Item = (String, String)> + '_> {
        Box::new(self.envs.iter().flat_map(|env| env.non_system_envs()))
    }
}

impl CompositeEnvironment {
    pub fn new(envs: Vec<Arc<dyn Environment>>) -> Self {
        Self { envs }
    }
}
