use super::mapped_arena::SerializableSlabIndex;
use super::predicate::ColumnIdPathLink;

use super::types::GqlTypeModifier;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrderByParameter {
    pub name: String,
    pub type_name: String,
    pub type_id: SerializableSlabIndex<OrderByParameterType>,
    pub type_modifier: GqlTypeModifier,

    /// How does this parameter relates with the parent parameter?
    /// For example for parameter used as {order_by: {venue1: {id: Desc}}}, we will have following column links:
    /// id: Some((<the venues.id column>, None))
    /// venue1: Some((<the concerts.venue1_id column>, <the venues.id column>))
    /// order_by: None
    pub column_path_link: Option<ColumnIdPathLink>,
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
