mod request;

use futures::StreamExt;
use lambda_runtime::{Error, LambdaEvent};
use payas_resolver_core::request_context::{ContextParsingError, RequestContext};
use payas_server_core::{OperationsPayload, SystemContext};
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
    system_context: Arc<SystemContext>,
) -> Result<Value, Error> {
    let request = LambdaRequest::new(&event);
    let request_context = RequestContext::parse_context(&request, vec![]);

    let body = event.payload["body"].clone();

    match request_context {
        Ok(request_context) => {
            let operations_payload: Option<OperationsPayload> = body
                .as_str()
                .and_then(|body_string| serde_json::from_str(body_string).ok());

            match operations_payload {
                Some(operations_payload) => {
                    let (stream, headers) = payas_server_core::resolve::<Error>(
                        operations_payload,
                        &system_context,
                        request_context,
                    )
                    .await;

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

                None => Ok(error_msg("Invalid query payload", 400)),
            }
        }

        Err(err) => {
            let response = match err {
                ContextParsingError::Unauthorized => error_msg("Unauthorized", 401),
                ContextParsingError::Malformed => error_msg("Malformed header", 400),
                _ => error_msg("Unknown error", 401),
            };

            Ok(response)
        }
    }
}
