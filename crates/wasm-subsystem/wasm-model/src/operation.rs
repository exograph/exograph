use std::ops::Deref;

use serde::{Deserialize, Serialize};
use subsystem_model_util::operation::{ServiceMutation, ServiceQuery};

pub use subsystem_model_util::operation::OperationReturnType;

#[derive(Serialize, Deserialize, Debug)]
pub struct WasmQuery(pub ServiceQuery);

impl Deref for WasmQuery {
    type Target = ServiceQuery;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WasmMutation(pub ServiceMutation);

impl Deref for WasmMutation {
    type Target = ServiceMutation;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
