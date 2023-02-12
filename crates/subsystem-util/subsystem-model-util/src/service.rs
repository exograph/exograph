use serde::{Deserialize, Serialize};

use crate::access::Access;

use super::{
    operation::{ServiceMutation, ServiceQuery},
    types::{ServiceType, ServiceTypeModifier},
};
use core_model::{mapped_arena::SerializableSlabIndex, types::OperationReturnType};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceMethod {
    pub name: String,
    pub script: SerializableSlabIndex<Script>,
    pub operation_kind: ServiceMethodType,
    pub is_exported: bool,
    pub arguments: Vec<Argument>,
    pub access: Access,
    pub return_type: OperationReturnType<ServiceType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Script {
    pub path: String,
    pub script: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Argument {
    pub name: String,
    pub type_id: SerializableSlabIndex<ServiceType>,
    pub modifier: ServiceTypeModifier,
    pub is_injected: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceMethodType {
    Query(SerializableSlabIndex<ServiceQuery>),
    Mutation(SerializableSlabIndex<ServiceMutation>),
}
