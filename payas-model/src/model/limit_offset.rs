use serde::{Deserialize, Serialize};

use super::GqlType;

use super::mapped_arena::SerializableSlabIndex;
use super::types::GqlTypeModifier;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LimitParameter {
    pub name: String,
    pub type_name: String,
    pub type_id: SerializableSlabIndex<GqlType>,
    pub type_modifier: GqlTypeModifier,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OffsetParameter {
    pub name: String,
    pub type_name: String,
    pub type_id: SerializableSlabIndex<GqlType>,
    pub type_modifier: GqlTypeModifier,
}
