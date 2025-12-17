#[cfg(not(target_family = "wasm"))]
use crate::env_const::{EXO_JWT_SECRET, EXO_OIDC_URL};
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::OnceCell;
use tracing::warn;

use crate::context::RequestContext;
use crate::context::context_extractor::ContextExtractor;
use crate::context::error::ContextExtractionError;
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

        let claims = self
            .extracted_claims
            .get_or_try_init(|| async {
                self.extract_authentication(
                    request_context.get_head(),
                    request_context.system_context.jwt_authenticator,
                )
                .await
            })
            .await?;

        // Debug: Log the full JWT claims structure
        eprintln!(
            "[JWT Extractor] Full JWT claims: {}",
            serde_json::to_string_pretty(&claims).unwrap_or_else(|_| "<invalid json>".to_string())
        );
        eprintln!("[JWT Extractor] Extracting key: {}", key);

        // Support both '.' and '/' as path separators
        // For keys with '/', split ONLY on the last '/' to handle keys like "https://hasura.io/jwt/claims"
        let current_value = if key.contains('/') {
            eprintln!("[JWT Extractor] Using '/' separator - splitting on last '/' only");
            if let Some(last_slash_pos) = key.rfind('/') {
                let (base_key, nested_key) = key.split_at(last_slash_pos);
                let nested_key = &nested_key[1..]; // Skip the '/'
                eprintln!(
                    "[JWT Extractor] Base key: {}, Nested key: {}",
                    base_key, nested_key
                );

                // First get the base key (e.g., "https://hasura.io/jwt/claims")
                if let Some(base_value) = claims.get(base_key) {
                    eprintln!("[JWT Extractor] Found base value: {:?}", base_value);
                    // Then get the nested key (e.g., "x-hasura-user-id")
                    base_value.get(nested_key)
                } else {
                    eprintln!("[JWT Extractor] Base key not found");
                    None
                }
            } else {
                claims.get(key)
            }
        } else {
            // Use '.' separator and navigate through all parts
            eprintln!("[JWT Extractor] Using '.' separator");
            key.split('.').fold(Some(claims), |value, part| {
                eprintln!("[JWT Extractor] Navigating to part: {}", part);
                if let Some(value) = value {
                    let result = value.get(part);
                    eprintln!("[JWT Extractor] Result: {:?}", result);
                    result
                } else {
                    None
                }
            })
        };

        eprintln!(
            "[JWT Extractor] Final value for key '{}': {:?}",
            key, current_value
        );
        Ok(current_value.cloned())
    }
}
