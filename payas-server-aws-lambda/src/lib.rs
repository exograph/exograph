pub mod request_context;

use lambda_http::{
    http::{Error, StatusCode},
    IntoResponse, Response,
};
use payas_server_core::{OperationsExecutor, OperationsPayload};

use futures::stream::StreamExt;
use request_context::{ContextProducerError, LambdaRequestContextProducer};
use serde_json::Value;

macro_rules! error_msg {
    ($msg:literal) => {
        concat!("{\"errors\": [{\"message\":\"", $msg, "\"}]}")
            .as_bytes()
            .to_vec()
    };
}

// #[post("/")]
pub async fn resolve(
    req: lambda_http::Request,
    body: Value,
    executor: OperationsExecutor,
    context_processor: LambdaRequestContextProducer,
) -> Result<impl IntoResponse, Error> {
    let request_context = context_processor.generate_request_context(&req);

    match request_context {
        Ok(request_context) => {
            let operations_payload: Result<OperationsPayload, _> = serde_json::from_value(body);

            match operations_payload {
                Ok(operations_payload) => {
                    let stream = payas_server_core::resolve::<Error>(
                        &executor,
                        operations_payload,
                        request_context,
                    )
                    .await;

                    let bytes = stream
                        .map(|chunks| chunks.unwrap())
                        .collect::<Vec<_>>()
                        .await;

                    let bytes: Vec<u8> =
                        bytes.into_iter().flat_map(|bytes| bytes.to_vec()).collect();

                    Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/json")
                        .body(bytes)
                }
                Err(_) => {
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(error_msg!("Invalid query payload"));
                }
            }
        }

        Err(err) => {
            let (message, base_response) = match err {
                ContextProducerError::Unauthorized => (
                    error_msg!("Unauthorized"),
                    Response::builder().status(StatusCode::UNAUTHORIZED),
                ),
                ContextProducerError::Malformed => (
                    error_msg!("Malformed header"),
                    Response::builder().status(StatusCode::BAD_REQUEST),
                ),
                ContextProducerError::Unknown => (
                    error_msg!("Unknown error"),
                    Response::builder().status(StatusCode::UNAUTHORIZED),
                ),
            };

            base_response
                .header("Content-Type", "application/json")
                .body(message)
        }
    }
}
