use serde::{Deserialize, Serialize};

use super::{
    mapped_arena::SerializableSlabIndex,
    service::{Argument, Script},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Interceptor {
    pub name: String,
    pub script: SerializableSlabIndex<Script>,
    pub interceptor_kind: InterceptorKind,
    pub arguments: Vec<Argument>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum InterceptorKind {
    Before,
    After,
    Around,
}
