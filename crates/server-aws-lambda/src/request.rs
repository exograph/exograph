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
// Reference: https://docs.aws.amazon.com/lambda/latest/dg/urls-invocation.html#urls-payloads
pub struct LambdaRequest<'a> {
    event: &'a LambdaEvent<Value>,
}

impl<'a> LambdaRequest<'a> {
    pub fn new(event: &'a LambdaEvent<Value>) -> LambdaRequest<'a> {
        LambdaRequest { event }
    }
}

impl LambdaRequest<'_> {
    fn http_payload(&self) -> &Value {
        &self.event.payload["requestContext"]["http"]
    }
}

impl RequestHead for LambdaRequest<'_> {
    fn get_headers(&self, key: &str) -> Vec<String> {
        if key == "cookie" {
            let cookies_payload = self.event.payload["cookies"].clone();

            if cookies_payload.is_null() {
                return vec![];
            }

            let cookies_array = cookies_payload.as_array();

            return match cookies_array {
                Some(cookies_array) => {
                    if cookies_array.is_empty() {
                        vec![]
                    } else {
                        let cookies_string = cookies_array
                            .into_iter()
                            .flat_map(|cookie| cookie.as_str().map(|s| s.to_string()))
                            .collect::<Vec<String>>()
                            .join("; ");

                        vec![cookies_string]
                    }
                }
                None => vec![],
            };
        }

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
        self.http_payload()
            .get("sourceIp")
            .and_then(|source_ip| source_ip.as_str())
            .and_then(|str| str.parse::<std::net::IpAddr>().ok())
    }

    fn get_method(&self) -> http::Method {
        match self.http_payload()["method"].as_str() {
            Some(method) => match method {
                "GET" => http::Method::GET,
                "POST" => http::Method::POST,
                "PUT" => http::Method::PUT,
                "DELETE" => http::Method::DELETE,
                "PATCH" => http::Method::PATCH,
                "OPTIONS" => http::Method::OPTIONS,
                "HEAD" => http::Method::HEAD,
                _ => http::Method::GET,
            },
            None => http::Method::GET,
        }
    }

    fn get_path(&self) -> String {
        self.http_payload()["path"]
            .as_str()
            .unwrap_or("/")
            .to_string()
    }

    fn get_query(&self) -> serde_json::Value {
        self.event
            .payload
            .get("queryStringParameters")
            .cloned()
            .unwrap_or_default()
    }
}
