use serde::{Serialize, Deserialize};

use super::{GqlType, GqlTypeModifier, mapped_arena::SerializableSlabIndex};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArgumentParameter {
    pub name: String,
    pub type_name: String,
    pub type_modifier: GqlTypeModifier,
    pub type_id: Option<SerializableSlabIndex<ArgumentParameterType>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArgumentParameterType {
    pub name: String,
    pub actual_type_id: Option<SerializableSlabIndex<GqlType>>,
}
