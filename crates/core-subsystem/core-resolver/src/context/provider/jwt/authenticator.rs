use std::cell::RefCell;
#[cfg(not(target_family = "wasm"))]
use std::env;

#[cfg(not(target_family = "wasm"))]
use common::env_const::{EXO_JWT_SECRET, EXO_OIDC_URL};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde_json::Value;

use thiserror::Error;
use tracing::error;

use crate::context::error::ContextExtractionError;
use crate::context::request::Request;

#[cfg(feature = "oidc")]
use super::oidc::Oidc;

// we spawn many resolvers concurrently in integration tests
thread_local! {
    pub static LOCAL_JWT_SECRET: RefCell<Option<String>> =  const { RefCell::new(None) };
    #[cfg(feature = "oidc")]
    pub static LOCAL_OIDC_URL: RefCell<Option<String>> =  const {RefCell::new(None) };
}

/// Authenticator with information about how to validate JWT tokens
/// It can be either a secret or a OIDC url
pub enum JwtAuthenticator {
    Secret(String),
    #[cfg(feature = "oidc")]
    Oidc(Oidc),
}

#[derive(Debug, Error)]
pub(super) enum JwtAuthenticationError {
    #[error("Invalid token")]
    Invalid,
    #[error("Expired token")]
    Expired,
    #[error("Delegate error: {0}")]
    Delegate(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
pub enum JwtConfigurationError {
    #[error("Invalid setup: {0}")]
    InvalidSetup(String),

    #[error("JWT configuration error `{message}`")]
    Configuration {
        message: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl JwtAuthenticator {
    pub async fn new_from_env() -> Result<Option<Self>, JwtConfigurationError> {
        let secret = LOCAL_JWT_SECRET.with(|local_jwt_secret| {
            local_jwt_secret
                .borrow()
                .clone()
                .or_else(|| {
                    #[cfg(not(target_family = "wasm"))]
                    {
                        env::var(EXO_JWT_SECRET).ok()
                    }

                    #[cfg(target_family = "wasm")]
                    {
                        None
                    }
                })
        });

        #[cfg(feature = "oidc")]
        let oidc_url =
            LOCAL_OIDC_URL.with(|url| url.borrow().clone().or_else(|| env::var(EXO_OIDC_URL).ok()));

        #[cfg(not(feature = "oidc"))]
        let oidc_url: Option<String> = None;

        match (secret, oidc_url) {
            (Some(secret), None) => Ok(Some(JwtAuthenticator::Secret(secret))),
            #[cfg(feature = "oidc")]
            (None, Some(oidc_url)) => Ok(Some(JwtAuthenticator::Oidc(Oidc::new(oidc_url).await?))),
            #[cfg(feature = "oidc")]
            (Some(_), Some(_)) => {
                Err(JwtConfigurationError::InvalidSetup(format!("Both {EXO_JWT_SECRET} and {EXO_OIDC_URL} are set. Only one of them can be set at a time")))
            }
            (None, None) => Ok(None),
            #[cfg(not(feature = "oidc"))]
            (_, Some(_)) => unreachable!()
        }
    }

    async fn validate_jwt(&self, token: &str) -> Result<Value, JwtAuthenticationError> {
        fn map_jwt_error(error: jsonwebtoken::errors::Error) -> JwtAuthenticationError {
            match error.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    JwtAuthenticationError::Expired
                }
                _ => JwtAuthenticationError::Invalid,
            }
        }

        match self {
            JwtAuthenticator::Secret(secret) => Ok(decode::<Value>(
                token,
                &DecodingKey::from_secret(secret.as_ref()),
                &Validation::default(),
            )
            .map_err(map_jwt_error)?
            .claims),
            #[cfg(feature = "oidc")]
            JwtAuthenticator::Oidc(oidc) => oidc.validate(token).await.map_err(|err| match err {
                oidc_jwt_validator::ValidationError::ValidationFailed(err) => map_jwt_error(err),
                err => {
                    error!("Error validating JWT: {}", err);
                    JwtAuthenticationError::Invalid
                }
            }),
        }
    }

    /// Extract authentication form the "Authorization" header with a bearer token
    /// The claim is deserialized into an opaque json `Value`, which will be eventually be mapped to
    /// the declared user context model
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
                .map_err(|err| match &err {
                    JwtAuthenticationError::Invalid => ContextExtractionError::Unauthorized,
                    JwtAuthenticationError::Expired => {
                        ContextExtractionError::ExpiredAuthentication
                    }
                    JwtAuthenticationError::Delegate(err) => {
                        error!("Error validating JWT: {}", err);
                        ContextExtractionError::Unauthorized
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
