use id_arena::Id;

use super::{
    order::OrderByParameter,
    predicate::PredicateParameter,
    types::{ModelType, ModelTypeModifier},
};

#[derive(Debug, Clone)]
pub struct Query {
    pub name: String,
    pub predicate_param: Option<PredicateParameter>,
    pub order_by_param: Option<OrderByParameter>,
    pub return_type: OperationReturnType,
}

#[derive(Debug, Clone)]
pub struct Mutation {
    pub name: String,
    pub kind: MutationKind,
    pub return_type: OperationReturnType,
}

#[derive(Debug, Clone)]
pub enum MutationKind {
    Create(MutationDataParameter),
    Delete(PredicateParameter),
    Update {
        data_param: MutationDataParameter,
        predicate_param: PredicateParameter,
    },
}

#[derive(Debug, Clone)]
pub struct MutationDataParameter {
    pub name: String,
    pub type_name: String,
    pub type_id: Id<ModelType>,
}

#[derive(Debug, Clone)]
pub struct OperationReturnType {
    pub type_id: Id<ModelType>,
    pub type_name: String,
    pub type_modifier: ModelTypeModifier,
}
