use serde::{Deserialize, Serialize};

use super::column_id::ColumnId;
use super::GqlType;

use super::mapped_arena::SerializableSlabIndex;
use super::types::GqlTypeModifier;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ColumnDependency {
    pub column_id: ColumnId,
    pub parent_type_id: SerializableSlabIndex<GqlType>,
    pub parent_field_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PredicateParameter {
    pub name: String,      // For example, "where", "and", "id", "venue", etc.
    pub type_name: String, // For example, "ConcertFilter", "IntFilter" (we need to keep this only for introspection, which doesn't have access to the ModelSystem)
    pub type_modifier: GqlTypeModifier,
    pub type_id: SerializableSlabIndex<PredicateParameterType>, // Type id of the parameter type (e.g. IntFilter, StringFilter)
    // The column associated with the parameter along its parent
    // For example for parameter used as {where: {venue1: {id: {eq: 1}}}}, we will have following column dependencies:
    // eq: None
    // id: Some((<the id column in the venues table>, <Venue>, "id"))
    // venue1: Some((<the venues column in the concerts table><Concert>, "venue1"))
    // where: None
    pub column_dependency: Option<ColumnDependency>, // Column id of the parameter (e.g. for a field, the column id is the field's column id)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PredicateParameterType {
    pub name: String,
    pub kind: PredicateParameterTypeKind,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PredicateParameterTypeKind {
    ImplicitEqual,                     // {id: 3}
    Operator(Vec<PredicateParameter>), // {lt: ..,gt: ..} such as IntFilter
    Composite {
        field_params: Vec<PredicateParameter>, // {where: {id: .., name: ..}} such as AccountFilter
        logical_op_params: Vec<PredicateParameter>, // logical operator predicates like `and: [{name: ..}, {id: ..}]`
    },
}
