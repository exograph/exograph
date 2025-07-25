// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

mod composite;
mod dot;
mod map;
mod system;

pub use composite::CompositeEnvironment;
pub use dot::DotEnvironment;
pub use map::MapEnvironment;
pub use system::SystemEnvironment;

pub trait Environment: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;

    fn enabled(&self, key: &str, default_value: bool) -> Result<bool, EnvError> {
        match self.get(key) {
            Some(value) => match value.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" | "enabled" | "enable" => Ok(true),
                "false" | "0" | "no" | "off" | "disabled" | "disable" => Ok(false),
                _ => Err(EnvError::InvalidBoolean {
                    key: key.to_string(),
                    value,
                }),
            },
            None => Ok(default_value),
        }
    }

    fn get_or_else(&self, key: &str, default_value: &str) -> String {
        self.get(key).unwrap_or(default_value.to_string())
    }

    fn get_list(&self, key: &str, default_value: Vec<String>) -> Vec<String> {
        self.get(key)
            .map(|value| value.split(',').map(|s| s.trim().into()).collect())
            .unwrap_or(default_value)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EnvError {
    #[error(
        "Invalid value for {key}: {value}. Expected true, 1, yes, on, enabled, enable OR false, 0, no, off, disabled, disable"
    )]
    InvalidBoolean { key: String, value: String },

    #[error("Invalid env value {env_value} for {env_key}: {message}")]
    InvalidEnum {
        env_key: &'static str,
        env_value: String,
        message: String,
    },
}
