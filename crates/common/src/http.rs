// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Mutex;

use http::StatusCode;
use serde_json::Value;

use bytes::Bytes;
use futures::Stream;
use std::{collections::HashMap, net::IpAddr, pin::Pin};

pub trait RequestPayload {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync);

    /// Take away the body from the request payload and return it
    ///
    /// Except for the first call, the return value will be `Value::Null`
    fn take_body(&self) -> Value;
}

type PinnedStream<E> = Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>;

#[derive(Debug, Clone)]
pub struct Headers {
    inner: Vec<(String, String)>,
}

impl Headers {
    pub fn new() -> Self {
        Self { inner: vec![] }
    }

    pub fn from_vec(vec: Vec<(String, String)>) -> Self {
        let mut headers = vec![];
        for (key, value) in vec {
            headers.push((key.to_lowercase(), value));
        }
        Self { inner: headers }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.inner
            .iter()
            .find(|(k, _)| k == &key.to_lowercase())
            .map(|(_, v)| v.clone())
    }

    pub fn insert(&mut self, key: String, value: String) {
        self.inner.push((key.to_lowercase(), value));
    }

    pub fn remove(&mut self, key: &str) {
        self.inner.retain(|(k, _)| k != &key.to_lowercase());
    }
}

impl IntoIterator for Headers {
    type Item = (String, String);
    type IntoIter = std::vec::IntoIter<(String, String)>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

pub struct ResponsePayload {
    pub body: ResponseBody,
    pub headers: Headers,
    pub status_code: StatusCode,
}

pub enum ResponseBody {
    Stream(PinnedStream<std::io::Error>),
    Bytes(Vec<u8>),
    Redirect(String),
    None,
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

    fn get_path(&self) -> String;
    fn get_query(&self) -> serde_json::Value;

    fn get_method(&self) -> http::Method;
}

pub fn strip_leading_slash(path: &str) -> String {
    strip_leading(path, "/").to_string()
}

pub fn strip_leading(path: &str, leading: &str) -> String {
    path.strip_prefix(leading).unwrap_or(path).to_string()
}

impl ResponseBody {
    pub async fn to_string(self) -> Result<String, ResponseBodyError> {
        match self {
            ResponseBody::Stream(stream) => {
                use futures::StreamExt;

                let bytes = stream
                    .map(|chunks| chunks.unwrap())
                    .collect::<Vec<_>>()
                    .await;

                let bytes: Vec<u8> = bytes.into_iter().flat_map(|bytes| bytes.to_vec()).collect();

                Ok(std::str::from_utf8(&bytes)?.to_string())
            }
            ResponseBody::Bytes(bytes) => Ok(std::str::from_utf8(&bytes)?.to_string()),
            ResponseBody::Redirect(..) => Err(ResponseBodyError::UnexpectedRedirect),
            ResponseBody::None => Ok("".to_string()),
        }
    }

    pub async fn to_json(self) -> Result<Value, ResponseBodyError> {
        let body = self.to_string().await?;
        Ok(serde_json::from_str(&body)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResponseBodyError {
    #[error("Response stream is not UTF-8")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Unexpected redirect")]
    UnexpectedRedirect,
    #[error("Response stream is not valid JSON")]
    Json(#[from] serde_json::Error),
}

pub struct MemoryRequestHead {
    headers: HashMap<String, Vec<String>>,
    cookies: HashMap<String, String>,
    method: http::Method,
    path: String,
    query: Value,
    ip: Option<String>,
}

impl MemoryRequestHead {
    pub fn new(
        headers: HashMap<String, Vec<String>>,
        cookies: HashMap<String, String>,
        method: http::Method,
        path: String,
        query: Value,
        ip: Option<String>,
    ) -> Self {
        Self {
            headers: headers
                .into_iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), v))
                .collect(),
            cookies,
            method,
            path,
            query,
            ip,
        }
    }

    pub fn add_header(&mut self, key: &str, value: &str) {
        self.headers
            .entry(key.to_string().to_ascii_lowercase())
            .or_default()
            .push(value.to_string());
    }
}

impl RequestHead for MemoryRequestHead {
    fn get_headers(&self, key: &str) -> Vec<String> {
        let key = key.to_ascii_lowercase();

        if key == "cookie" {
            return self
                .cookies
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
        } else {
            self.headers.get(&key).unwrap_or(&vec![]).clone()
        }
    }

    fn get_ip(&self) -> Option<std::net::IpAddr> {
        self.ip.as_ref().map(|ip| IpAddr::V4(ip.parse().unwrap()))
    }

    fn get_method(&self) -> http::Method {
        self.method.clone()
    }

    fn get_path(&self) -> String {
        self.path.clone()
    }

    fn get_query(&self) -> Value {
        self.query.clone()
    }
}

pub struct MemoryRequestPayload {
    body: Mutex<Value>,
    head: MemoryRequestHead,
}

impl MemoryRequestPayload {
    pub fn new(body: Value, head: MemoryRequestHead) -> Self {
        Self {
            body: Mutex::new(body),
            head,
        }
    }
}

impl RequestPayload for MemoryRequestPayload {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        &self.head
    }
    fn take_body(&self) -> Value {
        self.body.lock().unwrap().clone()
    }
}
