use super::{order::OrderByParameter, predicate::PredicateParameter, types::ModelTypeModifier};

#[derive(Debug, Clone)]
pub struct Query {
    pub name: String,
    pub predicate_parameter: Option<PredicateParameter>,
    pub order_by_param: Option<OrderByParameter>,
    pub return_type: OperationReturnType,
}

#[derive(Debug, Clone)]
pub struct OperationReturnType {
    pub type_name: String,
    pub type_modifier: ModelTypeModifier,
}
