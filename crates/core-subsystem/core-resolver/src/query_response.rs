// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde_json::Value as JsonValue;

#[derive(Debug, Clone)]
pub struct QueryResponse {
    pub body: QueryResponseBody,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub enum QueryResponseBody {
    Json(JsonValue),
    Raw(Option<String>),
}

impl QueryResponseBody {
    pub fn to_json(&self) -> Result<JsonValue, serde_json::Error> {
        match &self {
            QueryResponseBody::Json(val) => Ok(val.clone()),
            QueryResponseBody::Raw(raw) => {
                if let Some(raw) = raw {
                    serde_json::from_str(raw)
                } else {
                    Ok(JsonValue::Null)
                }
            }
        }
    }
}
