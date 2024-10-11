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

use common::{TestRequest, TestResponse};
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
        TestRequest {
            query: WHATS_MY_IP_QUERY,
            headers: HashMap::new(),
            ip: "1.2.3.4",
            cookies: vec![],
            method: http::Method::POST,
            path: "/graphql",
        },
        TestResponse {
            status_code: 200,
            headers: HashMap::new(),
            cookies: vec![],
            body: json!({

                "data": {
                    "whatsMyIp": "1.2.3.4"
                }
            }),
        },
    )
    .await;
}

#[tokio::test]
async fn no_headers() {
    common::test_query(
        TestRequest {
            query: REQUEST_CONTEXT_QUERY,
            headers: HashMap::new(),
            ip: "1.2.3.4",
            cookies: vec![],
            method: http::Method::POST,
            path: "/graphql",
        },
        TestResponse {
            status_code: 200,
            headers: HashMap::new(),
            cookies: vec![],
            body: json!({
                "data": {
                    "requestContext": {
                        "apiKey": null,
                        "clientKey": null,
                        "sessionId": null
                    }
                }
            }),
        },
    )
    .await;
}

#[tokio::test]
async fn with_request_headers() {
    common::test_query(
        TestRequest {
            query: REQUEST_CONTEXT_QUERY,
            headers: HashMap::from([("api-key", "apiKeyValue"), ("client-key", "clientKeyValue")]),
            ip: "1.2.3.4",
            cookies: vec![],
            method: http::Method::POST,
            path: "/graphql",
        },
        TestResponse {
            status_code: 200,
            headers: HashMap::new(),
            cookies: vec![],
            body: json!({
                "data": {
                    "requestContext": {
                        "apiKey": "apiKeyValue",
                        "clientKey": "clientKeyValue",
                        "sessionId": null
                    }
                }
            }),
        },
    )
    .await;
}

#[tokio::test]
async fn with_request_cookies() {
    common::test_query(
        TestRequest {
            query: REQUEST_CONTEXT_QUERY,
            headers: HashMap::new(),
            ip: "1.2.3.4",
            cookies: vec![("session-id", "sessionIdValue")],
            method: http::Method::POST,
            path: "/graphql",
        },
        TestResponse {
            status_code: 200,
            headers: HashMap::new(),
            cookies: vec![],
            body: json!({
                "data": {
                    "requestContext": {
                        "apiKey": null,
                        "clientKey": null,
                        "sessionId": "sessionIdValue"
                    }
                }
            }),
        },
    )
    .await;
}

#[tokio::test]
async fn add_response_header() {
    common::test_query(
        TestRequest {
            query: "{ addResponseHeader(name: \"x-test\", value: \"x-test-value\") }",
            headers: HashMap::new(),
            ip: "1.2.3.4",
            cookies: vec![],
            method: http::Method::POST,
            path: "/graphql",
        },
        TestResponse {
            status_code: 200,
            headers: HashMap::from([("x-test", "x-test-value")]),
            cookies: vec![],
            body: json!({
                "data": {
                    "addResponseHeader": "ok"
                }
            }),
        },
    )
    .await;
}

#[tokio::test]
async fn set_cookie() {
    common::test_query(
        TestRequest {
            query: "{ addResponseCookie(name: \"x-test-cookie\", value: \"x-test-cookie-value\") }",
            headers: HashMap::new(),
            ip: "1.2.3.4",
            cookies: vec![],
            method: http::Method::POST,
            path: "/graphql",
        },
        TestResponse {
            status_code: 200,
            headers: HashMap::new(),
            cookies: vec![("x-test-cookie", "x-test-cookie-value")],
            body: json!({
                "data": {
                    "addResponseCookie": "ok"
                }
            }),
        },
    )
    .await;
}
