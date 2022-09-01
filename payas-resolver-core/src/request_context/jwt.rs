use std::env;

use crate::{
    request_context::{BoxedParsedContext, ParsedContext, RequestContext},
    ResolveOperationFn,
};
use async_trait::async_trait;
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{decode, DecodingKey, TokenData, Validation};
use serde_json::Value;

use super::{ContextParsingError, Request};

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
        request: &dyn Request,
    ) -> Result<Value, JwtAuthenticationError> {
        let jwt_token = request
            .get_header("Authorization")
            .and_then(|auth_token| auth_token.strip_prefix("Bearer ").map(|t| t.to_owned()));

        match jwt_token {
            Some(jwt_token) => self
                .validate_jwt(&jwt_token)
                .map(|v| v.claims)
                .map_err(|err| match &err.kind() {
                    ErrorKind::InvalidSignature => JwtAuthenticationError::TamperedToken,
                    ErrorKind::ExpiredSignature => JwtAuthenticationError::ExpiredToken,
                    _ => JwtAuthenticationError::Unknown,
                }),
            None => {
                // Either the "Authorization" header was absent or the next token wasn't "Bearer"
                // It is not an error to have no authorization header, since that indicates an anonymous user
                // and there may be queries allowed for such users.
                Ok(serde_json::Value::Null)
            }
        }
    }

    pub fn parse_context(
        &self,
        request: &dyn Request,
    ) -> Result<BoxedParsedContext, ContextParsingError> {
        let jwt_claims = self.extract_authentication(request).map_err(|e| match e {
            JwtAuthenticationError::ExpiredToken | JwtAuthenticationError::TamperedToken => {
                ContextParsingError::Unauthorized
            }

            JwtAuthenticationError::Unknown => ContextParsingError::Malformed,
        })?;

        Ok(Box::new(ParsedJwtContext { jwt_claims }))
    }
}

pub struct ParsedJwtContext {
    jwt_claims: Value,
}

#[async_trait]
impl ParsedContext for ParsedJwtContext {
    fn annotation_name(&self) -> &str {
        "jwt"
    }

    async fn extract_context_field<'r>(
        &self,
        key: &str,
        _resolver: &ResolveOperationFn<'r>,
        _request_context: &'r RequestContext<'r>,
        _request: &'r (dyn Request + Send + Sync),
    ) -> Option<Value> {
        self.jwt_claims.get(key).cloned()
    }
}
