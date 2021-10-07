use serde::{Deserialize, Serialize};

use super::{
    mapped_arena::SerializableSlabIndex, operation::OperationReturnType, GqlType, GqlTypeModifier,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Service {
    pub name: String,
    pub methods: Vec<ServiceMethod>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceMethod {
    pub name: String,
    pub operation: ServiceMethodType,
    pub arguments: Vec<ServiceMethodArgument>,
    pub return_type: Option<OperationReturnType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceMethodType {
    Query,
    Mutation,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceMethodArgument {
    pub name: String,
    pub type_name: String,
    pub type_id: SerializableSlabIndex<GqlType>,
    pub type_modifier: GqlTypeModifier,
}
