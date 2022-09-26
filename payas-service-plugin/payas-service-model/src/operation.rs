use std::fmt::Debug;

use payas_core_model::mapped_arena::SerializableSlabIndex;

use payas_core_model::type_normalization::{Operation, Parameter, TypeModifier};
use serde::{Deserialize, Serialize};

use super::model::ModelServiceSystem;
use super::types::ServiceType;
use super::{argument::ArgumentParameter, service::ServiceMethod, types::ServiceTypeModifier};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceQuery {
    pub name: String,
    pub method_id: Option<SerializableSlabIndex<ServiceMethod>>,
    pub argument_param: Vec<ArgumentParameter>,
    pub return_type: OperationReturnType,
}

impl GraphQLOperation for ServiceQuery {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_query(&self) -> bool {
        true
    }
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

impl GraphQLOperation for ServiceMutation {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_query(&self) -> bool {
        false
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationReturnType {
    pub type_id: SerializableSlabIndex<ServiceType>,
    pub type_name: String,
    pub type_modifier: ServiceTypeModifier,
}

impl OperationReturnType {
    pub fn typ<'a>(&self, system: &'a ModelServiceSystem) -> &'a ServiceType {
        &system.service_types[self.type_id]
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
