// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(target_os = "linux")]

use std::collections::HashMap;

use serde_json::json;

mod common;

const REQUEST_CONTEXT_QUERY: &str = r#"query { 
    requestContext { 
        apiKey
        clientKey
        sessionId
    }
}"#;

const WHATS_MY_IP_QUERY: &str = r#"query { 
    whatsMyIp
}"#;

#[tokio::test]
async fn whats_my_ip() {
    common::test_query(
        WHATS_MY_IP_QUERY,
        HashMap::new(),
        "1.2.3.4",
        vec![],
        http::Method::POST,
        "/graphql",
        json!({
            "isBase64Encoded": false,
            "statusCode": 200,
            "headers": {},
            "multiValueHeaders": {"content-type": ["application/json"]},
            "body": serde_json::to_string(&json!({
                "data": {
                    "whatsMyIp": "1.2.3.4"
                }
            })).unwrap()
        }),
    )
    .await;
}

#[tokio::test]
async fn no_headers() {
    common::test_query(
        REQUEST_CONTEXT_QUERY,
        HashMap::new(),
        "1.2.3.4",
        vec![],
        http::Method::POST,
        "/graphql",
        json!({
            "isBase64Encoded": false,
            "statusCode": 200,
            "headers": {},
            "multiValueHeaders": {"content-type": ["application/json"]},
            "body": serde_json::to_string(&json!({
                "data": {
                    "requestContext": {
                        "apiKey": null,
                        "clientKey": null,
                        "sessionId": null
                    }
                }
            })).unwrap()
        }),
    )
    .await;
}

#[tokio::test]
async fn with_request_headers() {
    common::test_query(
        REQUEST_CONTEXT_QUERY,
        HashMap::from([("api-key", "apiKeyValue"), ("client-key", "clientKeyValue")]),
        "1.2.3.4",
        vec![],
        http::Method::POST,
        "/graphql",
        json!({
            "isBase64Encoded": false,
            "statusCode": 200,
            "headers": {},
            "multiValueHeaders": {"content-type": ["application/json"]},
            "body": serde_json::to_string(&json!({
                "data": {
                    "requestContext": {
                        "apiKey": "apiKeyValue",
                        "clientKey": "clientKeyValue",
                        "sessionId": null
                    }
                }
            })).unwrap()
        }),
    )
    .await;
}

#[tokio::test]
async fn with_request_cookies() {
    common::test_query(
        REQUEST_CONTEXT_QUERY,
        HashMap::from([]),
        "1.2.3.4",
        vec![("session-id", "sessionIdValue")],
        http::Method::POST,
        "/graphql",
        json!({
            "isBase64Encoded": false,
            "statusCode": 200,
            "headers": {},
            "multiValueHeaders": {"content-type": ["application/json"]},
            "body": serde_json::to_string(&json!({
                "data": {
                    "requestContext": {
                        "apiKey": null,
                        "clientKey": null,
                        "sessionId": "sessionIdValue"
                    }
                }
            })).unwrap()
        }),
    )
    .await;
}
