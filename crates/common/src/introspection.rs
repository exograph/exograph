use exo_env::Environment;

use crate::{env_const::EXO_INTROSPECTION, EnvError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntrospectionMode {
    Enabled,  // Introspection queries are allowed (typically dev/yolo mode)
    Disabled, // Introspection queries are not allowed (typically in production)
    Only,     // Only introspection queries are allowed (to support "exo playground")
}

pub fn introspection_mode(env: &dyn Environment) -> Result<IntrospectionMode, EnvError> {
    match env.get(EXO_INTROSPECTION) {
        Some(e) => match e.to_lowercase().as_str() {
            "true" | "enabled" | "1" => Ok(IntrospectionMode::Enabled),
            "false" | "disabled" => Ok(IntrospectionMode::Disabled),
            "only" => Ok(IntrospectionMode::Only),
            _ => Err(EnvError::InvalidEnum {
                env_key: EXO_INTROSPECTION,
                env_value: e,
                message: "Must be set to either true, enabled, 1, false, disabled, or only"
                    .to_string(),
            }),
        },

        None => Ok(IntrospectionMode::Disabled),
    }
}
