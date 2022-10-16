use serde::{Deserialize, Serialize};

/// A path representing context selection such as `AuthContext.role`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AccessContextSelection {
    Context(String),                             // for example, `AuthContext`
    Select(Box<AccessContextSelection>, String), // for example, `AuthContext.role`
}
