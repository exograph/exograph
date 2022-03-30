pub mod request_context_processor;

use actix_web::web::Bytes;
use actix_web::{web, Error, HttpRequest, HttpResponse, Responder};

use payas_server_core::SystemInfo;

use request_context_processor::{ContextProcessorError, RequestContextProcessor};
use serde_json::Value;

macro_rules! error_msg {
    ($msg:literal) => {
        concat!("{\"errors\": [{\"message\":\"", $msg, "\"}]}").as_bytes()
    };
}

pub async fn resolve(
    req: HttpRequest,
    body: web::Json<Value>,
    system_info: web::Data<SystemInfo>,
    context_processor: web::Data<RequestContextProcessor>,
) -> impl Responder {
    let request_context = context_processor.generate_request_context(&req);

    match request_context {
        Ok(request_context) => {
            let system_info = system_info.as_ref();

            let operation_name = body["operationName"].as_str();
            let query_str = body["query"].as_str().unwrap();
            let variables = body["variables"].as_object();

            let stream = payas_server_core::resolve::<Error>(
                system_info,
                operation_name,
                query_str,
                variables,
                request_context,
            )
            .await;
            HttpResponse::Ok()
                .content_type("application/json")
                .streaming(Box::pin(stream))
        }

        Err(err) => {
            let (message, mut base_response) = match err {
                ContextProcessorError::Unauthorized => {
                    (error_msg!("Unauthorized"), HttpResponse::Unauthorized())
                }
                ContextProcessorError::Malformed => {
                    (error_msg!("Malformed header"), HttpResponse::BadRequest())
                }
                ContextProcessorError::Unknown => {
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
