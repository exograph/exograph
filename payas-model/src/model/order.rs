use super::{column_id::ColumnId, mapped_arena::SerializableSlabIndex};

use super::types::GqlTypeModifier;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrderByParameter {
    pub name: String,
    pub type_name: String,
    pub type_id: SerializableSlabIndex<OrderByParameterType>,
    pub type_modifier: GqlTypeModifier,
    pub column_id: Option<ColumnId>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrderByParameterType {
    pub name: String,
    pub kind: OrderByParameterTypeKind,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OrderByParameterTypeKind {
    Primitive,
    Composite { parameters: Vec<OrderByParameter> },
}
