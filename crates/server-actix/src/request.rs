// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use actix_web::{dev::ConnectionInfo, http::header::HeaderMap, HttpRequest};
use core_resolver::http::RequestHead;

pub struct ActixRequestHead {
    // we cannot refer to HttpRequest directly, as it holds an Rc (and therefore does
    // not impl Send or Sync)
    //
    // request: &'a actix_web::HttpRequest,
    headers: HeaderMap,
    connection_info: ConnectionInfo,
}

impl ActixRequestHead {
    pub fn from_request(req: HttpRequest) -> ActixRequestHead {
        ActixRequestHead {
            headers: req.headers().clone(),
            connection_info: req.connection_info().clone(),
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
}
