use std::fmt::Debug;

use async_graphql_parser::types::Type;
use core_model::mapped_arena::SerializableSlabIndex;
use core_model::type_normalization::{Operation, Parameter};
use core_model::types::OperationReturnType;
use serde::{Deserialize, Serialize};

use super::types::ServiceType;
use super::{argument::ArgumentParameter, service::ServiceMethod};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceQuery {
    pub name: String,
    pub method_id: Option<SerializableSlabIndex<ServiceMethod>>,
    pub argument_param: Vec<ArgumentParameter>,
    pub return_type: OperationReturnType<ServiceType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceMutation {
    pub name: String,
    pub method_id: Option<SerializableSlabIndex<ServiceMethod>>,
    pub argument_param: Vec<ArgumentParameter>,
    pub return_type: OperationReturnType<ServiceType>,
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

    fn return_type(&self) -> Type {
        (&self.return_type).into()
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

    fn return_type(&self) -> Type {
        (&self.return_type).into()
    }
}
