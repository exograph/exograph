mod request;

use actix_web::web::Bytes;
use actix_web::{web, Error, HttpRequest, HttpResponse, Responder};

use payas_core_resolver::request_context::{ContextParsingError, RequestContext};
use payas_server_core::{OperationsPayload, SystemContext};
use request::ActixRequest;
use serde_json::Value;

macro_rules! error_msg {
    ($msg:literal) => {
        concat!("{\"errors\": [{\"message\":\"", $msg, "\"}]}").as_bytes()
    };
}

pub async fn resolve(
    req: HttpRequest,
    body: web::Json<Value>,
    system_context: web::Data<SystemContext>,
) -> impl Responder {
    let request = ActixRequest::from_request(req);
    let request_context = RequestContext::parse_context(&request, vec![]);

    match request_context {
        Ok(request_context) => {
            let operations_payload: Result<OperationsPayload, _> =
                serde_json::from_value(body.into_inner());

            match operations_payload {
                Ok(operations_payload) => {
                    let (stream, headers) = payas_server_core::resolve::<Error>(
                        operations_payload,
                        system_context.as_ref(),
                        request_context,
                    )
                    .await;

                    let mut builder = HttpResponse::Ok();
                    builder.content_type("application/json");

                    for header in headers.into_iter() {
                        builder.append_header(header);
                    }

                    builder.streaming(Box::pin(stream))
                }
                Err(_) => HttpResponse::BadRequest().body(error_msg!("Invalid query payload")),
            }
        }

        Err(err) => {
            let (message, mut base_response) = match err {
                ContextParsingError::Unauthorized => {
                    (error_msg!("Unauthorized"), HttpResponse::Unauthorized())
                }
                ContextParsingError::Malformed => {
                    (error_msg!("Malformed header"), HttpResponse::BadRequest())
                }
                _ => (error_msg!("Unknown error"), HttpResponse::Unauthorized()),
            };

            let error_message: Result<Bytes, Error> = Ok(Bytes::from_static(message));

            base_response
                .content_type("application/json")
                .streaming(Box::pin(futures::stream::once(async { error_message })))
        }
    }
}
