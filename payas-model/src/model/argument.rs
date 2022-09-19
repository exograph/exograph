use serde::{Deserialize, Serialize};

use super::{mapped_arena::SerializableSlabIndex, GqlType, GqlTypeModifier};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArgumentParameter {
    pub name: String,
    pub typ: ArgumentParameterType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArgumentParameterType {
    pub name: String,
    pub type_modifier: GqlTypeModifier,
    pub type_id: Option<SerializableSlabIndex<GqlType>>,
    pub is_primitive: bool,
}
