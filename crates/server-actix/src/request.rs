// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use actix_web::{dev::ConnectionInfo, http::header::HeaderMap, HttpRequest};
use common::http::RequestHead;

pub struct ActixRequestHead {
    // we cannot refer to HttpRequest directly, as it holds an Rc (and therefore does
    // not impl Send or Sync)
    //
    // request: &'a actix_web::HttpRequest,
    headers: HeaderMap,
    connection_info: ConnectionInfo,
    method: actix_web::http::Method,
    path: String,
    query: serde_json::Value,
}

impl ActixRequestHead {
    pub fn from_request(req: HttpRequest, query: serde_json::Value) -> ActixRequestHead {
        ActixRequestHead {
            headers: req.headers().clone(),
            connection_info: req.connection_info().clone(),
            method: req.method().clone(),
            path: req.path().to_string(),
            query,
        }
    }
}

impl RequestHead for ActixRequestHead {
    fn get_headers(&self, key: &str) -> Vec<String> {
        self.headers
            .get_all(key.to_lowercase())
            .map(|h| h.to_str().unwrap().to_string())
            .collect()
    }

    fn get_ip(&self) -> Option<std::net::IpAddr> {
        self.connection_info
            .realip_remote_addr()
            .and_then(|realip| realip.parse().ok())
    }

    fn get_method(&self) -> http::Method {
        // Actix uses http-0.2. However, the rest of the system uses
        // http-1.x, so we need to convert between the two.
        // Once Actix 5.x is released (which uses http-1.x), we can remove this mapping.
        match self.method {
            actix_web::http::Method::CONNECT => http::Method::CONNECT,
            actix_web::http::Method::GET => http::Method::GET,
            actix_web::http::Method::HEAD => http::Method::HEAD,
            actix_web::http::Method::OPTIONS => http::Method::OPTIONS,
            actix_web::http::Method::POST => http::Method::POST,
            actix_web::http::Method::PUT => http::Method::PUT,
            actix_web::http::Method::DELETE => http::Method::DELETE,
            actix_web::http::Method::PATCH => http::Method::PATCH,
            actix_web::http::Method::TRACE => http::Method::TRACE,
            _ => {
                tracing::error!("Unsupported method: {}", self.method);
                panic!("Unsupported method: {}", self.method);
            }
        }
    }

    fn get_path(&self) -> String {
        self.path.clone()
    }

    fn get_query(&self) -> serde_json::Value {
        self.query.clone()
    }
}
