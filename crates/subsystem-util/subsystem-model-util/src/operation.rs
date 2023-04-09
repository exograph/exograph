use std::fmt::Debug;

use async_graphql_parser::types::Type;
use core_model::mapped_arena::SerializableSlabIndex;
use core_model::type_normalization::{Operation, Parameter};
use core_model::types::OperationReturnType;
use serde::{Deserialize, Serialize};

use super::types::ModuleType;
use super::{argument::ArgumentParameter, module::ModuleMethod};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleQuery {
    pub name: String,
    pub method_id: Option<SerializableSlabIndex<ModuleMethod>>,
    pub argument_param: Vec<ArgumentParameter>,
    pub return_type: OperationReturnType<ModuleType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleMutation {
    pub name: String,
    pub method_id: Option<SerializableSlabIndex<ModuleMethod>>,
    pub argument_param: Vec<ArgumentParameter>,
    pub return_type: OperationReturnType<ModuleType>,
}

impl Operation for ModuleQuery {
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

impl Operation for ModuleMutation {
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
