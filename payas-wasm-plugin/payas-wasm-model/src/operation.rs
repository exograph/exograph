use std::ops::Deref;

use payas_plugin_model_util::operation::{ServiceMutation, ServiceQuery};
use serde::{Deserialize, Serialize};

pub use payas_plugin_model_util::operation::GraphQLOperation;
pub use payas_plugin_model_util::operation::OperationReturnType;

#[derive(Serialize, Deserialize, Debug)]
pub struct WasmQuery(pub ServiceQuery);

impl Deref for WasmQuery {
    type Target = ServiceQuery;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl GraphQLOperation for WasmQuery {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_query(&self) -> bool {
        true
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

impl GraphQLOperation for WasmMutation {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_query(&self) -> bool {
        false
    }
}
