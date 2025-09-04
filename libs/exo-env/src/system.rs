// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::Environment;

pub struct SystemEnvironment;

impl Environment for SystemEnvironment {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn non_system_envs(&self) -> Box<dyn Iterator<Item = (String, String)> + '_> {
        Box::new(std::iter::empty())
    }
}
