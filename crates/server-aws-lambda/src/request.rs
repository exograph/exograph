// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(target_os = "linux")]

use common::http::RequestHead;
use lambda_runtime::LambdaEvent;
use serde_json::Value;

// as lambda_runtime::LambdaEvent and core_resolver::request_context::Request are in different crates
// from this one, we must wrap the request with our own struct
pub struct LambdaRequest<'a> {
    event: &'a LambdaEvent<Value>,
    path: &'a str,
    method: http::Method,
    query: serde_json::Value,
}

impl<'a> LambdaRequest<'a> {
    pub fn new(event: &'a LambdaEvent<Value>) -> LambdaRequest<'a> {
        let method = match event.payload["httpMethod"].as_str() {
            Some(method) => match method {
                "GET" => http::Method::GET,
                "POST" => http::Method::POST,
                "PUT" => http::Method::PUT,
                "DELETE" => http::Method::DELETE,
                "PATCH" => http::Method::PATCH,
                "OPTIONS" => http::Method::OPTIONS,
                "HEAD" => http::Method::HEAD,
                "TRACE" => http::Method::TRACE,
                "CONNECT" => http::Method::CONNECT,
                _ => http::Method::GET,
            },
            None => http::Method::GET,
        };

        let path = event.payload["path"].as_str().unwrap_or("/");
        let query = event
            .payload
            .get("queryStringParameters")
            .cloned()
            .unwrap_or_default();

        LambdaRequest {
            event,
            path,
            method,
            query,
        }
    }
}

impl RequestHead for LambdaRequest<'_> {
    fn get_headers(&self, key: &str) -> Vec<String> {
        // handle "headers" field
        let mut headers: Vec<String> = self.event.payload["headers"]
            .as_object()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, v)| {
                if k == key {
                    v.as_str().map(str::to_string)
                } else {
                    None
                }
            })
            .collect();

        // handle "multiValueHeaders" field
        // https://aws.amazon.com/blogs/compute/support-for-multi-value-parameters-in-amazon-api-gateway/
        if let Some(header_map) = self.event.payload["multiValueHeaders"].as_object() {
            for (header, value) in header_map {
                if header == key {
                    if let Some(array) = value.as_array() {
                        for value in array.iter() {
                            if let Some(value) = value.as_str() {
                                headers.push(value.to_string())
                            }
                        }
                    }
                }
            }
        }

        headers
    }

    fn get_ip(&self) -> Option<std::net::IpAddr> {
        let event: &Value = &self.event.payload;

        event
            .get("requestContext")
            .and_then(|ctx| ctx.get("identity"))
            .and_then(|ident| ident.get("sourceIp"))
            .and_then(|source_ip| source_ip.as_str())
            .and_then(|str| str.parse::<std::net::IpAddr>().ok())
    }

    fn get_method(&self) -> &http::Method {
        &self.method
    }

    fn get_path(&self) -> &str {
        self.path
    }

    fn get_query(&self) -> serde_json::Value {
        self.query.clone()
    }
}
