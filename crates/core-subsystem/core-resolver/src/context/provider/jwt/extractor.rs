use std::sync::Arc;

use async_trait::async_trait;
#[cfg(not(target_family = "wasm"))]
use common::env_const::{EXO_JWT_SECRET, EXO_OIDC_URL};
use serde_json::Value;
use tokio::sync::OnceCell;
use tracing::warn;

use crate::context::context_extractor::ContextExtractor;
use crate::context::error::ContextExtractionError;
use crate::context::request::Request;
use crate::context::RequestContext;

use super::JwtAuthenticator;

pub struct JwtExtractor {
    jwt_authenticator: Arc<Option<JwtAuthenticator>>,
    extracted_claims: OnceCell<Value>,
}

impl JwtExtractor {
    pub fn new(jwt_authenticator: Arc<Option<JwtAuthenticator>>) -> Self {
        Self {
            jwt_authenticator,
            extracted_claims: OnceCell::new(),
        }
    }

    async fn extract_authentication(
        &self,
        request: &(dyn Request + Send + Sync),
    ) -> Result<Value, ContextExtractionError> {
        if let Some(jwt_authenticator) = self.jwt_authenticator.as_ref() {
            jwt_authenticator.extract_authentication(request).await
        } else {
            #[cfg(target_family = "wasm")]
            warn!("JWT secret or OIDC URL is not set, not parsing JWT tokens");

            #[cfg(not(target_family = "wasm"))]
            warn!(
                "{} or {} is not set, not parsing JWT tokens",
                EXO_JWT_SECRET, EXO_OIDC_URL
            );
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
            .get_or_try_init(|| async { self.extract_authentication(request).await })
            .await?
            .get(key)
            .cloned())
    }
}
