use serde::{Deserialize, Serialize};

use super::module::{Argument, Script};
use core_model::mapped_arena::SerializableSlabIndex;
use core_plugin_shared::interception::InterceptorKind;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Interceptor {
    pub module_name: String,
    pub method_name: String,
    pub script: SerializableSlabIndex<Script>,
    pub interceptor_kind: InterceptorKind,
    pub arguments: Vec<Argument>,
}
