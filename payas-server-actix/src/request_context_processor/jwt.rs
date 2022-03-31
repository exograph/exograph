use std::env;

use actix_web::http::header::Header;
use actix_web::HttpRequest;
use actix_web_httpauth::headers::authorization::Authorization;
use actix_web_httpauth::headers::authorization::Bearer;
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{decode, DecodingKey, TokenData, Validation};
use payas_server_core::request_context::BoxedParsedContext;
use payas_server_core::request_context::ParsedContextExtractor;
use serde_json::json;
use serde_json::Value;

use super::ContextProcessor;
use super::ContextProcessorError;

pub enum JwtAuthenticationError {
    ExpiredToken,
    TamperedToken,
    Unknown,
}

pub struct JwtAuthenticator {
    secret: String, // Shared secret for HS algorithms, public key for RSA/ES
}

const JWT_SECRET_PARAM: &str = "CLAY_JWT_SECRET";

impl JwtAuthenticator {
    pub fn new_from_env() -> Self {
        Self::new(env::var(JWT_SECRET_PARAM).ok().unwrap())
    }

    fn new(secret: String) -> Self {
        JwtAuthenticator { secret }
    }

    // TODO: Expand to work with extenral authentication providers such as auth0 (that require JWK support)
    fn validate_jwt(&self, token: &str) -> Result<TokenData<Value>, jsonwebtoken::errors::Error> {
        decode::<Value>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &Validation::default(),
        )
    }

    /// Extract authentication form the "Authorization" header with a bearer token
    /// The claim is deserialized into an opaque json `Value`, which will be eventually mapped
    /// to the declared user context model
    pub fn extract_authentication(
        &self,
        req: &HttpRequest,
    ) -> Result<Option<Value>, JwtAuthenticationError> {
        match Authorization::<Bearer>::parse(req) {
            Ok(auth) => {
                let scheme = auth.into_scheme();
                let token = scheme.token().as_ref();
                self.validate_jwt(token)
                    .map(|v| Some(v.claims))
                    .map_err(|err| match &err.kind() {
                        ErrorKind::InvalidSignature => JwtAuthenticationError::TamperedToken,
                        ErrorKind::ExpiredSignature => JwtAuthenticationError::ExpiredToken,
                        _ => JwtAuthenticationError::Unknown,
                    })
            }
            Err(_) => {
                // Either the "Authorization" header was absent or the next token wasn't "Bearer"
                // It is not an error to have no authorization header, since that indicates an anonymous user
                // and there may be queries allowed for such users.
                Ok(None)
            }
        }
    }
}

impl ContextProcessor for JwtAuthenticator {
    fn parse_context(
        &self,
        request: &HttpRequest,
    ) -> Result<BoxedParsedContext, ContextProcessorError> {
        let jwt_claims =
            self.extract_authentication(request)
                .map_err(|e| match e {
                    JwtAuthenticationError::ExpiredToken
                    | JwtAuthenticationError::TamperedToken => ContextProcessorError::Unauthorized,

                    JwtAuthenticationError::Unknown => ContextProcessorError::Malformed,
                })?
                .unwrap_or_else(|| json!({}));

        Ok(Box::new(ParsedJwtContext { jwt_claims }))
    }
}

struct ParsedJwtContext {
    jwt_claims: Value,
}

impl ParsedContextExtractor for ParsedJwtContext {
    fn annotation_name(&self) -> &str {
        "jwt"
    }

    fn extract_value(&self, key: &str) -> Option<Value> {
        self.jwt_claims.get(key).cloned()
    }
}
