use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
    access::Access,
    mapped_arena::SerializableSlabIndex,
    operation::{Mutation, OperationReturnType, Query},
    GqlType, GqlTypeModifier,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceMethod {
    pub name: String,
    pub module_path: PathBuf,
    pub operation_kind: ServiceMethodType,
    pub is_exported: bool,
    pub arguments: Vec<Argument>,
    pub access: Access,
    pub return_type: OperationReturnType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Argument {
    pub name: String,
    pub type_id: SerializableSlabIndex<GqlType>,
    pub modifier: GqlTypeModifier,
    pub is_injected: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceMethodType {
    Query(SerializableSlabIndex<Query>),
    Mutation(SerializableSlabIndex<Mutation>),
}
