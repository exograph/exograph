// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug)]
pub struct OperationsPayload {
    pub operation_name: Option<String>,
    pub query: Option<String>,
    pub variables: Option<Map<String, Value>>,
    pub query_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawOperationsPayload {
    #[serde(rename = "operationName")]
    pub operation_name: Option<String>,
    pub query: Option<String>,
    pub variables: Option<Map<String, Value>>,
    pub extensions: Option<Map<String, Value>>,
}

impl OperationsPayload {
    pub fn from_json(json: Value) -> Result<Self, serde_json::Error> {
        let raw_payload = serde_json::from_value::<RawOperationsPayload>(json);

        raw_payload.map(|raw_payload| {
            let query_hash = raw_payload.extensions.as_ref().and_then(|extensions| {
                extensions
                    .get("persistedQuery")
                    .and_then(|persisted_query| {
                        persisted_query
                            .get("sha256Hash")
                            .map(|hash| hash.as_str().unwrap().to_string())
                    })
            });

            OperationsPayload {
                operation_name: raw_payload.operation_name,
                query: raw_payload.query,
                variables: raw_payload.variables,
                query_hash,
            }
        })
    }

    pub fn to_json(&self) -> Result<Value, serde_json::Error> {
        let raw_payload = RawOperationsPayload {
            operation_name: self.operation_name.clone(),
            query: self.query.clone(),
            variables: self.variables.clone(),
            extensions: self.query_hash.as_ref().map(|query_hash| {
                let mut extensions = Map::new();
                let mut persisted_query = Map::new();
                persisted_query.insert("sha256Hash".to_string(), Value::String(query_hash.clone()));
                extensions.insert("persistedQuery".to_string(), Value::Object(persisted_query));
                extensions
            }),
        };

        serde_json::to_value(&raw_payload)
    }
}
