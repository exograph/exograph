use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
    mapped_arena::SerializableSlabIndex,
    operation::{Mutation, OperationReturnType, Query},
    GqlType, GqlTypeModifier,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceMethod {
    pub name: String,
    pub module_path: PathBuf,
    pub operation_kind: ServiceMethodType,
    pub arguments: Vec<MethodArgumentParameter>,
    pub return_type: Option<OperationReturnType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceMethodType {
    Query(SerializableSlabIndex<Query>),
    Mutation(SerializableSlabIndex<Mutation>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MethodArgumentParameter {
    pub name: String,
    pub type_name: String,
    pub type_modifier: GqlTypeModifier,
    pub type_id: SerializableSlabIndex<GqlType>,
}
