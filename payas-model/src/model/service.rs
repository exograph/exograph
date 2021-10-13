use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
    mapped_arena::SerializableSlabIndex,
    operation::{Mutation, OperationReturnType, Query},
    GqlType, GqlTypeModifier,
    argument::ArgumentParameter
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceMethod {
    pub name: String,
    pub module_path: PathBuf,
    pub operation_kind: ServiceMethodType,
    // FIXME: exported method flag
    pub arguments: Vec<(String, SerializableSlabIndex<GqlType>, GqlTypeModifier)>,   // FIXME: mark injected arguments 
    pub return_type: Option<OperationReturnType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceMethodType {
    Query(SerializableSlabIndex<Query>),
    Mutation(SerializableSlabIndex<Mutation>),
}

