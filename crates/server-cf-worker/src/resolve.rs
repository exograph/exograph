use serde_json::Value;

use worker::{Request as WorkerRequest, Response as WorkerResponse};

use common::http::{RequestHead, RequestPayload, ResponsePayload};

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

    let system_router = crate::init::get_system_router()?;

    let body_json: Value = worker_request
        .0
        .json()
        .await
        .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

    let request = WorkerRequestPayload {
        body: body_json,
        head: worker_request,
    };

    let ResponsePayload {
        stream,
        headers,
        status_code,
    } = system_router.route(request, false).await;

    let response = match stream {
        Some(stream) => {
            let mut response = WorkerResponse::from_stream(stream)
                .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

            for header in headers.into_iter() {
                response
                    .headers_mut()
                    .append(&header.0, &header.1)
                    .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
            }

            response.with_status(status_code.into())
        }
        None => WorkerResponse::builder()
            .with_status(status_code.into())
            .body(worker::ResponseBody::Empty),
    };

    web_sys::Response::try_from(response).map_err(|e| JsValue::from_str(&format!("{:?}", e)))
}
