pub mod jwt;

use actix_web::HttpRequest;
use anyhow::Context;
use payas_model::model::{ContextSource, ContextType};
use serde_json::{Map, Value};

use self::jwt::{JwtAuthenticationError, JwtAuthenticator};

pub struct ContextProcessor {
    jwt_authenticator: JwtAuthenticator,
}

pub enum ContextProcessorError {
    Jwt(JwtAuthenticationError),
    Unknown,
}

/// Represent a request context for a particular request
pub struct RequestContext {
    jwt_claims: Option<Value>,
}

impl ContextProcessor {
    pub fn new() -> ContextProcessor {
        ContextProcessor {
            jwt_authenticator: JwtAuthenticator::new_from_env(),
        }
    }

    /// Gathers and generates context information for requests (for example, JWT tokens, headers, envvars)
    ///
    /// The processor will authenticate information whenever possible (like the validity of JWT tokens)
    /// and throw an appropriate error on failure
    pub fn generate_request_context(
        &self,
        req: &HttpRequest,
    ) -> Result<RequestContext, ContextProcessorError> {
        let jwt_claims = self
            .jwt_authenticator
            .extract_authentication(req)
            .map_err(ContextProcessorError::Jwt)?;

        Ok(RequestContext { jwt_claims })
    }
}

impl RequestContext {
    /// Generates claims for a given ContextType in JSON form
    pub fn to_json_context(&self, context: &ContextType) -> anyhow::Result<Value> {
        let json_fields: anyhow::Result<Map<String, Value>> = context
            .fields
            .iter()
            .flat_map(|field| match &field.source {
                ContextSource::Jwt { claim } => self.jwt_claims.as_ref().map(|jwt_claims| {
                    Ok((
                        field.name.clone(),
                        jwt_claims
                            .get(claim)
                            .with_context(|| format!("{} not found in JWT token", claim))?
                            .clone(),
                    ))
                }),
            })
            .collect();

        Ok(Value::Object(json_fields?))
    }
}
