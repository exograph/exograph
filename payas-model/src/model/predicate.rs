use serde::{Deserialize, Serialize};

use super::column_id::ColumnId;

use super::mapped_arena::SerializableSlabIndex;
use super::types::GqlTypeModifier;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PredicateParameter {
    pub name: String,
    pub type_name: String,
    pub type_modifier: GqlTypeModifier,
    pub type_id: SerializableSlabIndex<PredicateParameterType>,
    pub column_id: Option<ColumnId>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PredicateParameterType {
    pub name: String,
    pub kind: PredicateParameterTypeKind,
}

type ModelPredicateParameters = Vec<PredicateParameter>;
type LogicalPredicateParameters = Vec<PredicateParameter>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PredicateParameterTypeKind {
    ImplicitEqual,                     // {id: 3}
    Operator(Vec<PredicateParameter>), // {lt: ..,gt: ..} such as IntFilter
    Composite {
        field_params: ModelPredicateParameters, // {where: {id: .., name: ..}} such as AccountFilter
        logical_op_params: LogicalPredicateParameters, // logical operator predicates like `and: [{name: ..}, {id: ..}]`
    },
}
