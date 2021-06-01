use super::column_id::ColumnId;

use super::types::GqlTypeModifier;
use id_arena::Id;

#[derive(Debug, Clone)]
pub struct OrderByParameter {
    pub name: String,
    pub type_name: String,
    pub type_id: Id<OrderByParameterType>,
    pub type_modifier: GqlTypeModifier,
    pub column_id: Option<ColumnId>,
}

#[derive(Debug, Clone)]
pub struct OrderByParameterType {
    pub name: String,
    pub kind: OrderByParameterTypeKind,
}

#[derive(Debug, Clone)]
pub enum OrderByParameterTypeKind {
    Primitive,
    Composite { parameters: Vec<OrderByParameter> },
}
