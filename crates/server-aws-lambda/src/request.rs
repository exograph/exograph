// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(target_os = "linux")]

use core_resolver::context::Request;
use lambda_runtime::LambdaEvent;
use serde_json::Value;

// as lambda_runtime::LambdaEvent and core_resolver::request_context::Request are in different crates
// from this one, we must wrap the request with our own struct
pub struct LambdaRequest<'a>(&'a LambdaEvent<Value>);

impl<'a> LambdaRequest<'a> {
    pub fn new(event: &'a LambdaEvent<Value>) -> LambdaRequest<'a> {
        LambdaRequest(event)
    }
}

impl Request for LambdaRequest<'_> {
    fn get_headers(&self, key: &str) -> Vec<String> {
        // handle "headers" field
        let mut headers: Vec<String> = self.0.payload["headers"]
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
        if let Some(header_map) = self.0.payload["multiValueHeaders"].as_object() {
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
        let event: &Value = &self.0.payload;

        event
            .get("requestContext")
            .and_then(|ctx| ctx.get("identity"))
            .and_then(|ident| ident.get("sourceIp"))
            .and_then(|source_ip| source_ip.as_str())
            .and_then(|str| str.parse::<std::net::IpAddr>().ok())
    }
}
