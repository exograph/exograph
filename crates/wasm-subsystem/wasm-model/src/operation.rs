use std::ops::Deref;

use serde::{Deserialize, Serialize};
use subsystem_model_util::operation::{ModuleMutation, ModuleQuery};

#[derive(Serialize, Deserialize, Debug)]
pub struct WasmQuery(pub ModuleQuery);

impl Deref for WasmQuery {
    type Target = ModuleQuery;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WasmMutation(pub ModuleMutation);

impl Deref for WasmMutation {
    type Target = ModuleMutation;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
