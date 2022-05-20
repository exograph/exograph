use std::env;

use async_trait::async_trait;
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{decode, DecodingKey, TokenData, Validation};
use payas_server_core::request_context::ParsedContext;
use payas_server_core::request_context::{BoxedParsedContext, RequestContext};
use payas_server_core::OperationsExecutor;
use serde_json::json;
use serde_json::Value;

use super::ContextProducerError;
use super::LambdaContextProducer;

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

    // TODO: Expand to work with external authentication providers such as auth0 (that require JWK support)
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
        req: &lambda_http::Request,
    ) -> Result<Option<Value>, JwtAuthenticationError> {
        let auth_token = req
            .headers()
            .get("Authorization")
            .ok_or(JwtAuthenticationError::Unknown)
            .and_then(|header| header.to_str().map_err(|_| JwtAuthenticationError::Unknown));

        let jwt_token = auth_token.and_then(|auth_token| {
            if auth_token.starts_with("Bearer ") {
                Err(JwtAuthenticationError::Unknown)
            } else {
                Ok(auth_token["Bearer ".len()..].trim())
            }
        });

        match jwt_token {
            Ok(jwt_token) => {
                let jwt_result = self.validate_jwt(jwt_token);

                match jwt_result {
                    Ok(v) => Ok(Some(v.claims)),
                    Err(err) => match &err.kind() {
                        ErrorKind::InvalidSignature => Err(JwtAuthenticationError::TamperedToken),
                        ErrorKind::ExpiredSignature => Err(JwtAuthenticationError::ExpiredToken),
                        _ => Err(JwtAuthenticationError::Unknown),
                    },
                }
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

impl LambdaContextProducer for JwtAuthenticator {
    fn parse_context(
        &self,
        request: &lambda_http::Request,
    ) -> Result<BoxedParsedContext, ContextProducerError> {
        let jwt_claims =
            self.extract_authentication(request)
                .map_err(|e| match e {
                    JwtAuthenticationError::ExpiredToken
                    | JwtAuthenticationError::TamperedToken => ContextProducerError::Unauthorized,

                    JwtAuthenticationError::Unknown => ContextProducerError::Malformed,
                })?
                .unwrap_or_else(|| json!({}));

        Ok(Box::new(ParsedJwtContext { jwt_claims }))
    }
}

struct ParsedJwtContext {
    jwt_claims: Value,
}

#[async_trait]
impl ParsedContext for ParsedJwtContext {
    fn annotation_name(&self) -> &str {
        "jwt"
    }

    async fn extract_context_field<'e>(
        &'e self,
        value: &str,
        _executor: &'e OperationsExecutor,
        _request_context: &'e RequestContext<'e>,
    ) -> Option<Value> {
        self.jwt_claims.get(value).cloned()
    }
}
