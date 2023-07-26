// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

mod request;

use actix_web::web::Bytes;
use actix_web::{web, Error, HttpRequest, HttpResponse, Responder};

use core_resolver::context::{ContextExtractionError, RequestContext};
use core_resolver::system_resolver::SystemResolver;
use core_resolver::OperationsPayload;
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
    system_resolver: web::Data<SystemResolver>,
) -> impl Responder {
    let request = ActixRequest::from_request(req);
    let request_context = RequestContext::new(&request, vec![], system_resolver.as_ref());

    match request_context {
        Ok(request_context) => {
            let operations_payload: Result<OperationsPayload, _> =
                serde_json::from_value(body.into_inner());

            match operations_payload {
                Ok(operations_payload) => {
                    let (stream, headers) = resolver::resolve::<Error>(
                        operations_payload,
                        system_resolver.as_ref(),
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
                ContextExtractionError::Unauthorized => {
                    (error_msg!("Unauthorized"), HttpResponse::Unauthorized())
                }
                ContextExtractionError::Malformed => {
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
