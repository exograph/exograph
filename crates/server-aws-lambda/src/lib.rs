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

use core_resolver::system_resolver::SystemResolver;
use futures::StreamExt;
use lambda_runtime::{Error, LambdaEvent};
use request::LambdaRequest;
use serde_json::{json, Value};
use std::sync::Arc;

fn error_msg(message: &str, status_code: usize) -> Value {
    let body = format!(r#"{{ "errors": [{{"message": "{message}"}}] }}"#);

    json!({
        "isBase64Encoded": false,
        "statusCode": status_code,
        "body": body
    })
}

pub async fn resolve(
    event: LambdaEvent<Value>,
    system_resolver: Arc<SystemResolver>,
) -> Result<Value, Error> {
    let request = LambdaRequest::new(&event);

    let body = event.payload["body"].clone();

    let operations_payload_json: Option<Value> = body
        .as_str()
        .and_then(|body_string| serde_json::from_str(body_string).ok());

    match operations_payload_json {
        Some(operations_payload_json) => {
            match resolver::resolve::<Error>(
                operations_payload_json,
                &system_resolver,
                &request,
                false,
            )
            .await
            {
                Ok((stream, headers)) => {
                    let bytes = stream
                        .map(|chunks| chunks.unwrap())
                        .collect::<Vec<_>>()
                        .await;

                    let bytes: Vec<u8> =
                        bytes.into_iter().flat_map(|bytes| bytes.to_vec()).collect();

                    // it would be nice to just pass `bytes` as the body,
                    // but lambda_http sets "isBase64Encoded" for the Lambda integration response if
                    // the body is not a string, and so our response gets base64'd if we do
                    let body_string = std::str::from_utf8(&bytes)
                        .expect("Response stream is not UTF-8")
                        .to_string();

                    Ok(json!({
                        "isBase64Encoded": false,
                        "statusCode": 200,
                        "headers": {},
                        "multiValueHeaders": headers
                            .into_iter()
                            .fold(json!({}), |mut acc, (k, v)| {
                                if let Some(value) = acc.get_mut(&k) {
                                    let array = value.as_array_mut().unwrap();
                                    array.push(v.into());
                                } else {
                                    let map = acc.as_object_mut().unwrap();
                                    map[&k] = v.into();
                                }

                                acc
                            }),
                        "body": body_string
                    }))
                }
                Err(_) => Ok(error_msg("Invalid query payload", 400)),
            }
        }

        None => Ok(error_msg("Invalid query payload", 400)),
    }
}
