use serde::{Deserialize, Serialize};

use super::column_id::ColumnId;
use super::GqlType;

use super::mapped_arena::SerializableSlabIndex;
use super::types::GqlTypeModifier;

/// The columns that need to form an equals predicate for forming a join.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JoinDependency {
    pub self_column_id: ColumnId,
    pub dependent_column_id: Option<ColumnId>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PredicateParameter {
    /// The name of the parameter. For example, "where", "and", "id", "venue", etc.
    pub name: String,
    /// The type name of the parameter.
    /// For example, "ConcertFilter", "IntFilter". We need to keep this only for introspection, which doesn't have access to the ModelSystem.
    /// We might find a way to avoid this, since given the model system and type_id of the parameter, we can get the type name.
    pub type_name: String,
    /// The type modifier of the parameter. For parameters such as "and", this will be a list.
    pub type_modifier: GqlTypeModifier,
    /// Type id of the parameter type. For example: IntFilter, StringFilter, etc.
    pub type_id: SerializableSlabIndex<PredicateParameterType>,

    /// For example for parameter used as {where: {venue1: {id: {eq: 1}}}}, we will have following column dependencies:
    /// eq: None
    /// id: Some((<the venues.id column>, None))
    /// venue1: Some((<the concerts.venue1_id column>, <the venues.id column>))
    /// where: None
    pub join_dependency: Option<JoinDependency>,

    /// The type this parameter is filtering on. For example, for ConcertFilter, this will be (the index of) the Concert.
    pub underlying_type_id: SerializableSlabIndex<GqlType>,
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
