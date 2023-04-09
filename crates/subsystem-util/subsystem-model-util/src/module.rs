use serde::{Deserialize, Serialize};

use crate::access::Access;

use super::{
    operation::{ModuleMutation, ModuleQuery},
    types::ModuleType,
};
use core_model::{
    mapped_arena::SerializableSlabIndex,
    types::{FieldType, OperationReturnType},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleMethod {
    pub name: String,
    pub script: SerializableSlabIndex<Script>,
    pub operation_kind: ModuleMethodType,
    pub is_exported: bool,
    pub arguments: Vec<Argument>,
    pub access: Access,
    pub return_type: OperationReturnType<ModuleType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Script {
    pub path: String,
    pub script: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Argument {
    pub name: String,
    pub type_id: FieldType<SerializableSlabIndex<ModuleType>>,
    pub is_injected: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ModuleMethodType {
    Query(SerializableSlabIndex<ModuleQuery>),
    Mutation(SerializableSlabIndex<ModuleMutation>),
}
