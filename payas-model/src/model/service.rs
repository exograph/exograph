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
    pub script: SerializableSlabIndex<Script>,
    pub operation_kind: ServiceMethodType,
    pub is_exported: bool,
    pub arguments: Vec<Argument>,
    pub access: Access,
    pub return_type: OperationReturnType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Script {
    pub path: String,
    pub script: Vec<u8>,
    pub script_kind: ScriptKind,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub enum ScriptKind {
    Deno,
    Wasm,
}

impl ScriptKind {
    pub fn from_script_name(script_name: &str) -> ScriptKind {
        if script_name.ends_with(".wasm") {
            ScriptKind::Wasm
        } else {
            ScriptKind::Deno
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Argument {
    pub name: String,
    pub type_id: SerializableSlabIndex<GqlType>,
    pub is_primitive: bool,
    pub modifier: GqlTypeModifier,
    pub is_injected: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceMethodType {
    Query(SerializableSlabIndex<Query>),
    Mutation(SerializableSlabIndex<Mutation>),
}
