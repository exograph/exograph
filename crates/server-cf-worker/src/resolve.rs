use http::StatusCode;
use serde_json::Value;

use worker::{Request as WorkerRequest, Response as WorkerResponse};

use common::http::{Headers, RequestHead, RequestPayload, ResponseBody, ResponsePayload};
use common::router::Router;

use wasm_bindgen::prelude::*;

struct WorkerRequestWrapper(WorkerRequest, String, Value);

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

    fn get_method(&self) -> &http::Method {
        match self.0.method() {
            worker::Method::Head => &http::Method::HEAD,
            worker::Method::Get => &http::Method::GET,
            worker::Method::Post => &http::Method::POST,
            worker::Method::Put => &http::Method::PUT,
            worker::Method::Patch => &http::Method::PATCH,
            worker::Method::Delete => &http::Method::DELETE,
            worker::Method::Options => &http::Method::OPTIONS,
            worker::Method::Connect => &http::Method::CONNECT,
            worker::Method::Trace => &http::Method::TRACE,
        }
    }

    fn get_path(&self) -> &str {
        &self.1
    }

    fn get_query(&self) -> Value {
        self.2.clone()
    }
}

struct WorkerRequestPayload {
    body: Value,
    head: WorkerRequestWrapper,
}

impl RequestPayload for WorkerRequestPayload {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        &self.head
    }

    fn take_body(&mut self) -> Value {
        self.body.take()
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
    let path = url.path().to_string();
    let query = url
        .query_pairs()
        .map(|q| {
            let (k, v) = q;
            (k.to_string(), v.to_string())
        })
        .collect();

    let mut worker_request =
        WorkerRequestWrapper(WorkerRequest::from(raw_request), path.into(), query);

    let body_json: Value = worker_request.0.json().await.unwrap_or(Value::Null);

    let mut request = WorkerRequestPayload {
        body: body_json,
        head: worker_request,
    };

    let system_router = crate::init::get_system_router()?;
    let response_payload = system_router.route(&mut request, true).await;

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
