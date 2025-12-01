// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(target_os = "linux")]

mod request;

use std::sync::{Arc, Mutex};

use common::{
    http::{Headers, RequestHead, RequestPayload, ResponseBody, ResponsePayload},
    router::{PlainRequestPayload, Router},
};
use futures::StreamExt;
use lambda_runtime::{Error, LambdaEvent};
use request::LambdaRequest;
use serde_json::{Value, json};

use system_router::SystemRouter;

struct AwsLambdaRequestPayload {
    head: LambdaRequest,
    body: Mutex<Value>,
}

impl RequestPayload for AwsLambdaRequestPayload {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        &self.head
    }

    fn take_body(&self) -> Value {
        self.body.lock().unwrap().take()
    }
}

pub async fn resolve(
    event: LambdaEvent<Value>,
    system_router: Arc<SystemRouter>,
) -> Result<Value, Error> {
    let body_str = event.payload["body"].as_str();
    let body = match body_str {
        Some(body_str) => serde_json::from_str(body_str)?,
        None => Value::Null,
    };

    let request_payload = AwsLambdaRequestPayload {
        head: LambdaRequest::new(event),
        body: Mutex::new(body),
    };

    let response_payload = system_router
        .route(&PlainRequestPayload::external(Box::new(request_payload)))
        .await;

    match response_payload {
        Some(ResponsePayload {
            body,
            headers,
            status_code,
        }) => {
            let (body_string, additional_headers) = match body {
                ResponseBody::Stream(stream) => {
                    let bytes = stream
                        .map(|chunks| chunks.unwrap())
                        .collect::<Vec<_>>()
                        .await;

                    let bytes: Vec<u8> =
                        bytes.into_iter().flat_map(|bytes| bytes.to_vec()).collect();

                    // it would be nice to just pass `bytes` as the body,
                    // but lambda_http sets "isBase64Encoded" for the Lambda integration response if
                    // the body is not a string, and so our response gets base64'd if we do
                    (
                        std::str::from_utf8(&bytes)
                            .expect("Response stream is not UTF-8")
                            .to_string(),
                        Headers::new(),
                    )
                }
                ResponseBody::Bytes(bytes) => (
                    std::str::from_utf8(&bytes)
                        .expect("Response bytes are not UTF-8")
                        .to_string(),
                    Headers::new(),
                ),
                ResponseBody::None => ("".to_string(), Headers::new()),
                ResponseBody::Redirect(url) => {
                    let redirect_headers =
                        Headers::from_vec(vec![(http::header::LOCATION.to_string(), url)]);
                    ("".to_string(), redirect_headers)
                }
            };

            let (cookie_headers, headers): (Vec<_>, Vec<_>) = headers
                .into_iter()
                .partition(|(name, _)| name == "set-cookie");

            let cookie_headers = cookie_headers.into_iter().map(|(_, v)| v.to_string());

            let all_headers: Vec<(String, String)> = headers
                .into_iter()
                .chain(additional_headers.into_iter())
                .collect();

            Ok(json!({
                "isBase64Encoded": false,
                "statusCode": status_code.as_u16(),
                "headers": {},
                "multiValueHeaders": all_headers
                    .into_iter()
                    .fold(json!({}), |mut acc, (k, v)| {
                        if let Some(value) = acc.get_mut(&k) {
                            let array = value.as_array_mut().unwrap();
                            array.push(v.into());
                        } else {
                            let map = acc.as_object_mut().unwrap();
                            map.insert(k, Value::Array(vec![v.into()]));
                        }

                        acc
                    }),
                "cookies": serde_json::Value::from(cookie_headers.map(|value| Value::String(value)).collect::<Vec<_>>()),
                "body": body_string
            }))
        }

        None => Ok(json!({
            "statusCode": 404,
            "body": ""
        })),
    }
}
