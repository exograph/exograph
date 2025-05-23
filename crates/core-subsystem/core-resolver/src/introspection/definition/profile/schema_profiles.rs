// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::schema_profile::{SchemaProfile, SchemaProfileSerialized};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub(super) struct SchemaProfilesSerialized {
    #[serde(flatten)]
    profiles: HashMap<String, SchemaProfileSerialized>,
}

#[derive(Debug, Default)]
pub struct SchemaProfiles {
    pub(super) profiles: HashMap<String, SchemaProfile>,
}

impl SchemaProfiles {
    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        let ser = serde_json::from_str::<SchemaProfilesSerialized>(value)?;

        Ok(ser.into())
    }

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

impl From<SchemaProfilesSerialized> for SchemaProfiles {
    fn from(serialized: SchemaProfilesSerialized) -> Self {
        SchemaProfiles {
            profiles: serialized
                .profiles
                .into_iter()
                .map(|(name, profile)| (name, profile.into()))
                .collect(),
        }
    }
}
