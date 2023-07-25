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
use tokio::sync::OnceCell;
use tracing::warn;

use crate::context::context_extractor::ContextExtractor;
use crate::context::error::ContextExtractionError;
use crate::context::request::Request;
use crate::context::RequestContext;

pub struct JwtExtractor {
    jwt_authenticator: Option<JwtAuthenticator>,
    extracted_claims: OnceCell<Value>,
}

impl JwtExtractor {
    pub fn new(jwt_authenticator: Option<JwtAuthenticator>) -> Self {
        Self {
            jwt_authenticator,
            extracted_claims: OnceCell::new(),
        }
    }

    fn extract_authentication(
        &self,
        request: &dyn Request,
    ) -> Result<Value, ContextExtractionError> {
        if let Some(jwt_authenticator) = &self.jwt_authenticator {
            jwt_authenticator.extract_authentication(request)
        } else {
            warn!("{EXO_JWT_SECRET} is not set, not parsing JWT tokens");
            Ok(serde_json::Value::Null)
        }
    }
}

#[async_trait]
impl ContextExtractor for JwtExtractor {
    fn annotation_name(&self) -> &str {
        "jwt"
    }

    async fn extract_context_field(
        &self,
        key: &str,
        _request_context: &RequestContext,
        request: &(dyn Request + Send + Sync),
    ) -> Result<Option<Value>, ContextExtractionError> {
        Ok(self
            .extracted_claims
            .get_or_try_init(|| futures::future::ready(self.extract_authentication(request)))
            .await?
            .get(key)
            .cloned())
    }
}

#[derive(Clone)]
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
    fn extract_authentication(
        &self,
        request: &dyn Request,
    ) -> Result<Value, ContextExtractionError> {
        let jwt_token = request
            .get_header("Authorization")
            .and_then(|auth_token| auth_token.strip_prefix("Bearer ").map(|t| t.to_owned()));

        match jwt_token {
            Some(jwt_token) => self
                .validate_jwt(&jwt_token)
                .map(|v| v.claims)
                .map_err(|err| match &err.kind() {
                    ErrorKind::InvalidSignature | ErrorKind::ExpiredSignature => {
                        ContextExtractionError::Unauthorized
                    }
                    _ => ContextExtractionError::Malformed,
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
