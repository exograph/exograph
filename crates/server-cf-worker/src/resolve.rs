use serde_json::Value;
use std::ops::{Deref, DerefMut};

use worker::{Request as WorkerRequest, Response as WorkerResponse};

use core_resolver::{context::Request, exchange::Exchange};

use wasm_bindgen::prelude::*;

struct WorkerRequestWrapper(WorkerRequest);

impl Deref for WorkerRequestWrapper {
    type Target = WorkerRequest;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WorkerRequestWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

unsafe impl Send for WorkerRequestWrapper {}
unsafe impl Sync for WorkerRequestWrapper {}

impl Request for WorkerRequestWrapper {
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
}
struct WorkerExchange {
    body: Value,
    request: WorkerRequestWrapper,
}

impl Exchange for WorkerExchange {
    fn get_request(&self) -> &(dyn Request + Send + Sync) {
        &self.request
    }

    fn take_body(&mut self) -> Value {
        self.body.take()
    }
}

pub async fn resolve(raw_request: web_sys::Request) -> Result<web_sys::Response, JsValue> {
    let mut worker_request = WorkerRequestWrapper(WorkerRequest::from(raw_request));

    let system_resolver = crate::init::get_system_resolver()?;

    let body_json: Value = worker_request
        .json()
        .await
        .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

    let exchange = WorkerExchange {
        body: body_json,
        request: worker_request,
    };

    match resolver::resolve::<JsValue>(exchange, system_resolver, false).await {
        Ok((stream, headers)) => {
            let mut response = WorkerResponse::from_stream(stream)
                .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

            response
                .headers_mut()
                .append("content-type", "application/json")
                .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
            for header in headers.into_iter() {
                response
                    .headers_mut()
                    .append(&header.0, &header.1)
                    .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
            }

            web_sys::Response::try_from(response)
                .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
        }
        Err(_) => {
            let response = WorkerResponse::builder().error("Invalid payload", 400)?;
            web_sys::Response::try_from(response)
                .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
        }
    }
}
