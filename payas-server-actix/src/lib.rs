pub mod request_context;
pub mod telemetry;

use actix_web::web::Bytes;
use actix_web::{post, web, Error, HttpRequest, HttpResponse, Responder};

use payas_server_core::{OperationsExecutor, OperationsPayload};

use request_context::{ActixRequestContextProducer, ContextProducerError};
use serde_json::Value;

macro_rules! error_msg {
    ($msg:literal) => {
        concat!("{\"errors\": [{\"message\":\"", $msg, "\"}]}").as_bytes()
    };
}

#[post("/")]
pub async fn resolve(
    req: HttpRequest,
    body: web::Json<Value>,
    executor: web::Data<OperationsExecutor>,
    context_processor: web::Data<ActixRequestContextProducer>,
) -> impl Responder {
    let request_context = context_processor.generate_request_context(&req, &executor);

    match request_context {
        Ok(request_context) => {
            let operations_payload: Result<OperationsPayload, _> =
                serde_json::from_value(body.into_inner());

            match operations_payload {
                Ok(operations_payload) => {
                    let stream = payas_server_core::resolve::<Error>(
                        executor.as_ref(),
                        operations_payload,
                        request_context,
                    )
                    .await;

                    HttpResponse::Ok()
                        .content_type("application/json")
                        .streaming(Box::pin(stream))
                }
                Err(_) => {
                    return HttpResponse::BadRequest().body(error_msg!("Invalid query payload"));
                }
            }
        }

        Err(err) => {
            let (message, mut base_response) = match err {
                ContextProducerError::Unauthorized => {
                    (error_msg!("Unauthorized"), HttpResponse::Unauthorized())
                }
                ContextProducerError::Malformed => {
                    (error_msg!("Malformed header"), HttpResponse::BadRequest())
                }
                ContextProducerError::Unknown => {
                    (error_msg!("Unknown error"), HttpResponse::Unauthorized())
                }
            };

            let error_message: Result<Bytes, Error> = Ok(Bytes::from_static(message));

            base_response
                .content_type("application/json")
                .streaming(Box::pin(futures::stream::once(async { error_message })))
        }
    }
}
