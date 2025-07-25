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

pub struct DotEnvironment {
    file_path: std::path::PathBuf,
    vars: std::sync::OnceLock<HashMap<String, String>>,
}

impl DotEnvironment {
    pub fn new<P: AsRef<std::path::Path>>(file_path: P) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            vars: std::sync::OnceLock::new(),
        }
    }

    fn load_vars(&self) -> &HashMap<String, String> {
        self.vars.get_or_init(|| {
            if !self.file_path.exists() {
                return HashMap::new();
            }

            let mut vars = HashMap::new();
            if let Ok(iter) = dotenvy::from_filename_iter(&self.file_path) {
                for (key, value) in iter.flatten() {
                    vars.insert(key, value);
                }
            }
            vars
        })
    }
}

impl Environment for DotEnvironment {
    fn get(&self, key: &str) -> Option<String> {
        let vars = self.load_vars();
        vars.get(key).cloned()
    }
}
