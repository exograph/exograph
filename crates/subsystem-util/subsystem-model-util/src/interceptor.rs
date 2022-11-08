use serde::{Deserialize, Serialize};

use super::service::{Argument, Script};
use core_model::mapped_arena::SerializableSlabIndex;
use core_plugin_shared::interception::InterceptorKind;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Interceptor {
    pub name: String,
    pub script: SerializableSlabIndex<Script>,
    pub interceptor_kind: InterceptorKind,
    pub arguments: Vec<Argument>,
}
