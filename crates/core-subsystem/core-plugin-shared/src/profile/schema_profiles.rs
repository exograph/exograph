// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::schema_profile::SchemaProfile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
pub struct SchemaProfiles {
    pub profiles: HashMap<String, SchemaProfile>,
}

impl SchemaProfiles {
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

impl IntoIterator for SchemaProfiles {
    type Item = (String, SchemaProfile);
    type IntoIter = std::collections::hash_map::IntoIter<String, SchemaProfile>;
    fn into_iter(self) -> Self::IntoIter {
        self.profiles.into_iter()
    }
}
