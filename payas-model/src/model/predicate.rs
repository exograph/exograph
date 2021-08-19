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
type BooleanPredicateParameters = Vec<PredicateParameter>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PredicateParameterTypeKind {
    ImplicitEqual,                     // {id: 3}
    Opeartor(Vec<PredicateParameter>), // {lt: ..,gt: ..} such as IntFilter
    Composite(ModelPredicateParameters, BooleanPredicateParameters), // {where: {id: .., name: ..}} such as AccountFilter
                                                                     // also includes boolean predicates like
                                                                     // {where: {
                                                                     //   and: [{name: ..}, {id: ..}]
                                                                     // }}
}
