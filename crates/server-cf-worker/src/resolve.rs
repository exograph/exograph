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

use worker::{Request as WorkerRequest, Response as WorkerResponse};

use common::http::{Headers, RequestHead, RequestPayload, ResponseBody, ResponsePayload};
use common::router::{PlainRequestPayload, Router};

use wasm_bindgen::prelude::*;

struct WorkerRequestWrapper(WorkerRequest, Value);

unsafe impl Send for WorkerRequestWrapper {}
unsafe impl Sync for WorkerRequestWrapper {}

impl RequestHead for WorkerRequestWrapper {
    fn get_headers(&self, key: &str) -> Vec<String> {
        self.0
            .headers()
            .get(key)
            .unwrap()
            .map(|v| vec![v])
            .unwrap_or_default()
    }

    fn get_ip(&self) -> Option<std::net::IpAddr> {
        None
    }

    fn get_method(&self) -> http::Method {
        // Convert via string to avoid additional variant (currently "REPORT") in worker::Method
        http::Method::from_bytes(self.0.method().as_ref().as_bytes()).unwrap_or(http::Method::GET)
    }

    fn get_path(&self) -> String {
        self.0.path()
    }

    fn get_query(&self) -> Value {
        self.1.clone()
    }
}

struct WorkerRequestPayload {
    body: Mutex<Value>,
    head: WorkerRequestWrapper,
}

impl RequestPayload for WorkerRequestPayload {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        &self.head
    }

    fn take_body(&self) -> Value {
        self.body.lock().unwrap().take()
    }
}

fn with_headers(mut response: WorkerResponse, headers: Headers) -> Result<WorkerResponse, JsValue> {
    for header in headers.into_iter() {
        response
            .headers_mut()
            .append(&header.0, &header.1)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
    }

    Ok(response)
}

pub async fn resolve(raw_request: web_sys::Request) -> Result<web_sys::Response, JsValue> {
    let url =
        url::Url::parse(&raw_request.url()).map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
    let query = url
        .query_pairs()
        .map(|q| {
            let (k, v) = q;
            (k.to_string(), v.to_string())
        })
        .collect();

    let mut worker_request = WorkerRequestWrapper(WorkerRequest::from(raw_request), query);

    let body_json: Value = worker_request.0.json().await.unwrap_or(Value::Null);

    let request = WorkerRequestPayload {
        body: Mutex::new(body_json),
        head: worker_request,
    };

    let system_router = crate::init::get_system_router()?;
    let response_payload = system_router
        .route(&PlainRequestPayload::external(Box::new(request)))
        .await;

    let response = match response_payload {
        Some(ResponsePayload {
            body,
            headers,
            status_code,
        }) => match body {
            ResponseBody::Stream(stream) => with_headers(
                WorkerResponse::from_stream(stream)
                    .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?
                    .with_status(status_code.into()),
                headers,
            )?,
            ResponseBody::Bytes(bytes) => with_headers(
                WorkerResponse::from_bytes(bytes)
                    .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?
                    .with_status(status_code.into()),
                headers,
            )?,
            ResponseBody::Redirect(url) => {
                let url = url::Url::parse(&url)
                    .map_err(|e| JsValue::from_str(&format!("Bad redirect url {:?}", e)))?;
                with_headers(
                    WorkerResponse::redirect(url)
                        .map_err(|e| JsValue::from_str(&format!("Failed to redirect {:?}", e)))?,
                    headers,
                )?
            }
            ResponseBody::None => with_headers(
                WorkerResponse::builder()
                    .with_status(status_code.into())
                    .body(worker::ResponseBody::Empty),
                headers,
            )?,
        },
        None => WorkerResponse::builder()
            .with_status(StatusCode::NOT_FOUND.into())
            .body(worker::ResponseBody::Empty),
    };

    web_sys::Response::try_from(response).map_err(|e| JsValue::from_str(&format!("{:?}", e)))
}
