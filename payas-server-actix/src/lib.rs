use actix_web::web::Bytes;
use actix_web::{web, Error, HttpRequest, HttpResponse, Responder};

use payas_server::SystemInfo;

use serde_json::Value;

pub mod authentication;

use crate::authentication::{JwtAuthenticationError, JwtAuthenticator};

macro_rules! error_msg {
    ($msg:literal) => {
        concat!("{\"errors\": [{\"message\":\"", $msg, "\"}]}").as_bytes()
    };
}

pub async fn resolve(
    req: HttpRequest,
    body: web::Json<Value>,
    system_info: web::Data<SystemInfo>,
    authenticator: web::Data<JwtAuthenticator>,
) -> impl Responder {
    let auth = authenticator.extract_authentication(req);

    match auth {
        Ok(claims) => {
            let system_info = system_info.as_ref();

            let operation_name = body["operationName"].as_str();
            let query_str = body["query"].as_str().unwrap();
            let variables = body["variables"].as_object();

            let stream = payas_server::resolve::<Error>(
                system_info,
                operation_name,
                query_str,
                variables,
                claims,
            )
            .await;
            HttpResponse::Ok()
                .content_type("application/json")
                .streaming(Box::pin(stream))
        }
        Err(err) => {
            let (message, mut base_response) = match err {
                JwtAuthenticationError::ExpiredToken => (
                    error_msg!("Expired JWT token"),
                    HttpResponse::Unauthorized(),
                ),
                JwtAuthenticationError::TamperedToken => {
                    // No need to reveal more info for a tampered token, so mark is as a generic bad request
                    (error_msg!("Unexpected error"), HttpResponse::BadRequest())
                }
                JwtAuthenticationError::Unknown => {
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
