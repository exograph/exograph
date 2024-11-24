#[cfg(not(target_family = "wasm"))]
use crate::env_const::{EXO_JWT_SECRET, EXO_OIDC_URL};
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::OnceCell;
use tracing::warn;

use crate::context::context_extractor::ContextExtractor;
use crate::context::error::ContextExtractionError;
use crate::context::RequestContext;
use crate::http::RequestHead;

use super::JwtAuthenticator;

pub struct JwtExtractor {
    extracted_claims: OnceCell<Value>,
}

impl JwtExtractor {
    pub fn new() -> Self {
        Self {
            extracted_claims: OnceCell::new(),
        }
    }

    async fn extract_authentication(
        &self,
        request_head: &(dyn RequestHead + Send + Sync),
        jwt_authenticator: &Option<JwtAuthenticator>,
    ) -> Result<Value, ContextExtractionError> {
        if let Some(jwt_authenticator) = jwt_authenticator.as_ref() {
            jwt_authenticator.extract_authentication(request_head).await
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
        request_context: &RequestContext,
    ) -> Result<Option<Value>, ContextExtractionError> {
        use crate::http::RequestPayload;

        Ok(self
            .extracted_claims
            .get_or_try_init(|| async {
                self.extract_authentication(
                    request_context.get_head(),
                    request_context.get_base_context().jwt_authenticator,
                )
                .await
            })
            .await?
            .get(key)
            .cloned())
    }
}
