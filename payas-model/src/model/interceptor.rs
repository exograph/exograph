use serde::{Deserialize, Serialize};

use super::{mapped_arena::SerializableSlabIndex, service::Script, GqlType, GqlTypeModifier};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Interceptor {
    pub name: String,
    pub script: SerializableSlabIndex<Script>,
    pub interceptor_kind: InterceptorKind,
    pub arguments: Vec<InterceptorArgument>,
}

// TODO: Could this be an enum, since we accept only a fixed set of arguments (such as `Operation`)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InterceptorArgument {
    pub name: String,
    pub type_id: SerializableSlabIndex<GqlType>,
    pub modifier: GqlTypeModifier,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum InterceptorKind {
    Before,
    After,
    Around,
}
