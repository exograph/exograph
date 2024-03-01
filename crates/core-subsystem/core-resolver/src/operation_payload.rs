// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Debug)]
pub struct OperationsPayload {
    pub operation_name: Option<String>,
    pub query: Option<String>,
    pub variables: Option<Map<String, Value>>,
    pub query_hash: Option<String>,
}

impl OperationsPayload {
    pub fn from_json(json: Value) -> Result<Self, serde_json::Error> {
        #[derive(Debug, Deserialize)]
        pub struct RawOperationsPayload {
            #[serde(rename = "operationName")]
            pub operation_name: Option<String>,
            pub query: Option<String>,
            pub variables: Option<Map<String, Value>>,
            pub extensions: Option<Map<String, Value>>,
        }

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
}
