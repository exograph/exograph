pub mod request_context;

use std::sync::Arc;

use futures::StreamExt;
use lambda_http::{http::StatusCode, Error, Response};
use payas_server_core::{OperationsPayload, SystemContext};

use request_context::{ContextProducerError, LambdaRequestContextProducer};

fn error_msg(message: &str) -> String {
    format!(r#"{{ "errors": [{{"message": "{message}"}}] }}"#)
}

pub async fn resolve(
    req: lambda_http::Request,
    system_context: Arc<SystemContext>,
    context_processor: Arc<LambdaRequestContextProducer>,
) -> Result<Response<String>, Error> {
    let request_context = context_processor.generate_request_context(&req);

    let (_, body) = req.into_parts();

    let body_string = match body {
        lambda_http::Body::Empty => todo!(),
        lambda_http::Body::Text(string) => string,
        lambda_http::Body::Binary(_) => todo!(),
    };

    match request_context {
        Ok(request_context) => {
            let operations_payload: Result<OperationsPayload, _> =
                serde_json::from_str(&body_string);

            match operations_payload {
                Ok(operations_payload) => {
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

                    let mut builder = Response::builder();
                    builder = builder.status(StatusCode::OK);
                    builder = builder.header("Content-Type", "application/json");

                    for header in headers.iter() {
                        builder = builder.header(&header.0, &header.1)
                    }

                    Ok(builder.body(body_string)?)
                }
                Err(_) => Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(error_msg("Invalid query payload"))?),
            }
        }

        Err(err) => {
            let (message, base_response) = match err {
                ContextProducerError::Unauthorized => (
                    error_msg("Unauthorized"),
                    Response::builder().status(StatusCode::UNAUTHORIZED),
                ),
                ContextProducerError::Malformed => (
                    error_msg("Malformed header"),
                    Response::builder().status(StatusCode::BAD_REQUEST),
                ),
                ContextProducerError::Unknown => (
                    error_msg("Unknown error"),
                    Response::builder().status(StatusCode::UNAUTHORIZED),
                ),
            };

            Ok(base_response
                .header("Content-Type", "application/json")
                .body(message)?)
        }
    }
}
