use std::fmt::Debug;

use core_model::mapped_arena::{SerializableSlab, SerializableSlabIndex};

use core_model::type_normalization::{Operation, Parameter, TypeModifier};
use serde::{Deserialize, Serialize};

use super::types::ServiceType;
use super::{argument::ArgumentParameter, service::ServiceMethod, types::ServiceTypeModifier};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceQuery {
    pub name: String,
    pub method_id: Option<SerializableSlabIndex<ServiceMethod>>,
    pub argument_param: Vec<ArgumentParameter>,
    pub return_type: OperationReturnType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceMutation {
    pub name: String,
    pub method_id: Option<SerializableSlabIndex<ServiceMethod>>,
    pub argument_param: Vec<ArgumentParameter>,
    pub return_type: OperationReturnType,
}

pub trait GraphQLOperation: Debug {
    fn name(&self) -> &str;

    fn is_query(&self) -> bool;
}

// TODO: This is nearly duplicated from the database version. We should consolidate them.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationReturnType {
    pub type_id: SerializableSlabIndex<ServiceType>,
    pub type_name: String,
    pub type_modifier: ServiceTypeModifier,
}

impl OperationReturnType {
    pub fn typ<'a>(&self, service_types: &'a SerializableSlab<ServiceType>) -> &'a ServiceType {
        &service_types[self.type_id]
    }
}

impl Operation for ServiceQuery {
    fn name(&self) -> &String {
        &self.name
    }

    fn parameters(&self) -> Vec<&dyn Parameter> {
        let mut params: Vec<&dyn Parameter> = vec![];

        for arg in self.argument_param.iter() {
            params.push(arg)
        }

        params
    }

    fn return_type_name(&self) -> &str {
        &self.return_type.type_name
    }

    fn return_type_modifier(&self) -> TypeModifier {
        (&self.return_type.type_modifier).into()
    }
}

impl Operation for ServiceMutation {
    fn name(&self) -> &String {
        &self.name
    }

    fn parameters(&self) -> Vec<&dyn Parameter> {
        self.argument_param
            .iter()
            .map(|param| {
                let param: &dyn Parameter = param;
                param
            })
            .collect()
    }

    fn return_type_name(&self) -> &str {
        &self.return_type.type_name
    }

    fn return_type_modifier(&self) -> TypeModifier {
        (&self.return_type.type_modifier).into()
    }
}
