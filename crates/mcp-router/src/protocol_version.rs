use std::fmt::{self, Display};

use crate::error::McpRouterError;

#[derive(Debug)]
pub enum ProtocolVersion {
    V2024_11_05,
    V2025_03_26,
    V2025_06_18,
}

impl Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::V2024_11_05 => write!(f, "2024-11-05"),
            Self::V2025_03_26 => write!(f, "2025-03-26"),
            Self::V2025_06_18 => write!(f, "2025-06-18"),
        }
    }
}

impl TryFrom<&str> for ProtocolVersion {
    type Error = McpRouterError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "2024-11-05" => Ok(Self::V2024_11_05),
            "2025-03-26" => Ok(Self::V2025_03_26),
            "2025-06-18" => Ok(Self::V2025_06_18),
            _ => Err(McpRouterError::InvalidProtocolVersion(s.to_string())),
        }
    }
}
