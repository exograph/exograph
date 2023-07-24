// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::cell::RefCell;
use std::env;

use async_trait::async_trait;
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{decode, DecodingKey, TokenData, Validation};
use serde_json::Value;
use tracing::warn;

use crate::context::error::ContextParsingError;
use crate::context::parsed_context::{BoxedParsedContext, ParsedContext};
use crate::context::request::Request;
use crate::context::RequestContext;

pub enum JwtAuthenticationError {
    ExpiredToken,
    TamperedToken,
    Unknown,
}

pub struct JwtAuthenticator {
    secret: String, // Shared secret for HS algorithms, public key for RSA/ES
}

const EXO_JWT_SECRET: &str = "EXO_JWT_SECRET";

// we spawn many resolvers concurrently in integration tests
thread_local! {
    pub static LOCAL_JWT_SECRET: RefCell<Option<String>> = RefCell::new(None);
}

impl JwtAuthenticator {
    pub fn new_from_env() -> Option<Self> {
        LOCAL_JWT_SECRET
            .with(|local_jwt_secret| {
                local_jwt_secret
                    .borrow()
                    .clone()
                    .or_else(|| env::var(EXO_JWT_SECRET).ok())
            })
            .map(Self::new)
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

    pub fn parse_context<'a>(
        me: Option<&Self>,
        request: &'a dyn Request,
    ) -> Result<BoxedParsedContext<'a>, ContextParsingError> {
        let jwt_claims = if let Some(jwt_authenticator) = me {
            jwt_authenticator
                .extract_authentication(request)
                .map_err(|e| match e {
                    JwtAuthenticationError::ExpiredToken
                    | JwtAuthenticationError::TamperedToken => ContextParsingError::Unauthorized,

                    JwtAuthenticationError::Unknown => ContextParsingError::Malformed,
                })?
        } else {
            warn!("{EXO_JWT_SECRET} is not set, not parsing JWT tokens");
            serde_json::Value::Null
        };

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
        _request_context: &RequestContext,
        _request: &(dyn Request + Send + Sync),
    ) -> Result<Option<Value>, ContextParsingError> {
        Ok(self.jwt_claims.get(key).cloned())
    }
}
