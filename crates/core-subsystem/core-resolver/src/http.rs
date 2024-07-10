// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use http::StatusCode;
use serde_json::Value;

use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

pub trait RequestPayload {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync);
    fn take_body(&mut self) -> Value;
}

type PinnedStream<E> = Pin<Box<dyn Stream<Item = Result<Bytes, E>>>>;
pub type Headers = Vec<(String, String)>;

pub struct ResponsePayload<E> {
    pub stream: Option<PinnedStream<E>>,
    pub headers: Headers,
    pub status_code: StatusCode,
}

/// Represents a HTTP request from which information can be extracted
pub trait RequestHead {
    // return all header values that have the following key
    fn get_headers(&self, key: &str) -> Vec<String>;

    // return the first header
    fn get_header(&self, key: &str) -> Option<String> {
        self.get_headers(&key.to_lowercase()).first().cloned()
    }

    // return the IP address used to make the request
    fn get_ip(&self) -> Option<std::net::IpAddr>;
}
