pub mod jwt;

use std::collections::HashMap;

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
    MalformedHeader,
    Unknown,
}

/// Represent a request context for a particular request
pub struct RequestContext {
    jwt_claims: Option<Value>,
    headers: HashMap<String, String>,
}

impl Default for ContextProcessor {
    fn default() -> Self {
        Self::new()
    }
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

        let headers: Result<HashMap<String, String>, ContextProcessorError> = req
            .headers()
            .iter()
            .map(|(header_name, header_value)| {
                let name = header_name.to_string().to_ascii_lowercase();
                let value = header_value
                    .to_str()
                    .map_err(|_| ContextProcessorError::MalformedHeader)?
                    .to_string();
                Ok((name, value))
            })
            .collect();

        Ok(RequestContext {
            jwt_claims,
            headers: headers?,
        })
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
                            .with_context(|| format!("Claim `{}` not found in JWT token", claim))?
                            .clone(),
                    ))
                }),

                ContextSource::Header { header } => self
                    .headers
                    .get(&header.to_ascii_lowercase()) // headers are case insensitive
                    .map(|header_value| {
                        Ok((field.name.clone(), Value::String(header_value.to_string())))
                    }),

                ContextSource::EnvironmentVariable { envvar } => std::env::var(envvar)
                    .ok()
                    .map(|envvar| Ok((field.name.clone(), Value::String(envvar)))),
            })
            .collect();

        Ok(Value::Object(json_fields?))
    }
}
