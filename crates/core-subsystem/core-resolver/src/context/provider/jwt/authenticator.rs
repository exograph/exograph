use std::cell::RefCell;
use std::env;

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{decode, DecodingKey, TokenData, Validation};
use serde_json::Value;
use tracing::warn;

use thiserror::Error;

use crate::context::error::ContextExtractionError;
use crate::context::request::Request;

use super::jwks::{JwksEndpoint, JwksExtractionError};

pub(super) const EXO_JWT_SECRET: &str = "EXO_JWT_SECRET";
pub(super) const EXO_JWKS_ENDPOINT: &str = "EXO_JWKS_ENDPOINT";

// we spawn many resolvers concurrently in integration tests
thread_local! {
    pub static LOCAL_JWT_SECRET: RefCell<Option<String>> = RefCell::new(None);
    pub static LOCAL_JWKS_URL: RefCell<Option<String>> = RefCell::new(None);
}

/// Authenticator with information about how to validate JWT tokens
/// It can be either a secret or a JWKS endpoint
pub enum JwtAuthenticator {
    Secret(String),
    Endpoint(JwksEndpoint),
}

#[derive(Debug, Error)]
pub(super) enum JwtAuthenticationError {
    #[error("Token extraction error `{0}`")]
    JsonWebToken(#[from] jsonwebtoken::errors::Error),

    #[error("JWKS extraction error `{0}`")]
    JwksExtractionError(#[from] JwksExtractionError),
}

impl JwtAuthenticator {
    pub fn new_from_env() -> Option<Self> {
        let secret = LOCAL_JWT_SECRET.with(|local_jwt_secret| {
            local_jwt_secret
                .borrow()
                .clone()
                .or_else(|| env::var(EXO_JWT_SECRET).ok())
        });

        let jwks_url = LOCAL_JWKS_URL.with(|url| {
            url.borrow()
                .clone()
                .or_else(|| env::var(EXO_JWKS_ENDPOINT).ok())
        });

        match (secret, jwks_url) {
            (Some(secret), None) => Some(JwtAuthenticator::Secret(secret)),
            (None, Some(jwks_url)) => Some(JwtAuthenticator::Endpoint(JwksEndpoint::new(jwks_url))),
            (Some(_), Some(_)) => {
                warn!("Both {EXO_JWT_SECRET} and {EXO_JWKS_ENDPOINT} are set. JWT authentication will not be enabled.");
                None
            }
            (None, None) => None,
        }
    }

    async fn validate_jwt(&self, token: &str) -> Result<TokenData<Value>, JwtAuthenticationError> {
        match self {
            JwtAuthenticator::Secret(secret) => Ok(decode::<Value>(
                token,
                &DecodingKey::from_secret(secret.as_ref()),
                &Validation::default(),
            )?),
            JwtAuthenticator::Endpoint(endpoint) => Ok(endpoint.decode_token(token).await?),
        }
    }

    /// Extract authentication form the "Authorization" header with a bearer token
    /// The claim is deserialized into an opaque json `Value`, which will be eventually mapped
    /// to the declared user context model
    pub(super) async fn extract_authentication(
        &self,
        request: &(dyn Request + Send + Sync),
    ) -> Result<Value, ContextExtractionError> {
        let jwt_token = request
            .get_header("Authorization")
            .and_then(|auth_token| auth_token.strip_prefix("Bearer ").map(|t| t.to_owned()));

        match jwt_token {
            Some(jwt_token) => self
                .validate_jwt(&jwt_token)
                .await
                .map(|v| v.claims)
                .map_err(|err| match &err {
                    JwtAuthenticationError::JsonWebToken(err) => match err.kind() {
                        ErrorKind::InvalidSignature | ErrorKind::ExpiredSignature => {
                            ContextExtractionError::Unauthorized
                        }
                        _ => ContextExtractionError::Malformed,
                    },
                    JwtAuthenticationError::JwksExtractionError(_) => {
                        warn!("Failed to process JWT using JWKS: {}", err);
                        ContextExtractionError::Malformed
                    }
                }),
            None => {
                // Either the "Authorization" header was absent or the next token wasn't "Bearer"
                // It is not an error to have no authorization header, since that indicates an anonymous user
                // and there may be queries allowed for such users.
                Ok(serde_json::Value::Null)
            }
        }
    }
}
