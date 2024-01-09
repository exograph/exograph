// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(target_os = "linux")]

use serde_json::json;

mod common;

#[tokio::test]
async fn test_basic_query() {
    common::test_query(
        serde_json::from_str(include_str!("basic_query_input.json")).unwrap(),
        include_str!("model.exo"),
        json!({
            "isBase64Encoded": false,
            "statusCode": 200,
            "headers": {},
            "multiValueHeaders": {},
            "body": "{\"errors\": [{\"message\":\"Operation failed\"}]}"
        }),
    )
    .await
}
