pub mod request_context;

use actix_web::web::Bytes;
use actix_web::{web, Error, HttpRequest, HttpResponse, Responder};

use payas_server_core::QueryExecutor;

use request_context::{ActixRequestContextProducer, ContextProducerError};
use serde_json::Value;

macro_rules! error_msg {
    ($msg:literal) => {
        concat!("{\"errors\": [{\"message\":\"", $msg, "\"}]}").as_bytes()
    };
}

pub async fn resolve(
    req: HttpRequest,
    body: web::Json<Value>,
    executor: web::Data<QueryExecutor>,
    context_processor: web::Data<ActixRequestContextProducer>,
) -> impl Responder {
    let request_context = context_processor.generate_request_context(&req);

    match request_context {
        Ok(request_context) => {
            let executor = executor.as_ref();

            let operation_name = body["operationName"].as_str();
            let query_str = body["query"].as_str().unwrap();
            let variables = body["variables"].as_object();

            let stream = payas_server_core::resolve::<Error>(
                executor,
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
