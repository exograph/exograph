use std::collections::HashMap;

use exo_env::Environment;
use wasm_bindgen::JsValue;
use worker::Hyperdrive;

pub(crate) struct WorkerEnvironment {
    env: worker::Env,
    additional_envs: HashMap<String, String>,
}

impl WorkerEnvironment {
    pub fn new(env: worker::Env, additional_envs: HashMap<String, String>) -> Self {
        Self {
            env,
            additional_envs,
        }
    }

    pub fn hyperdrive(&self, binding: &str) -> Result<Hyperdrive, JsValue> {
        Ok(self.env.hyperdrive(binding)?)
    }
}

impl Environment for WorkerEnvironment {
    fn get(&self, key: &str) -> Option<String> {
        self.env
            .var(key)
            .ok()
            .map(|binding| binding.to_string())
            .or(self.additional_envs.get(key).cloned())
    }

    fn non_system_envs(&self) -> Box<dyn Iterator<Item = (String, String)> + '_> {
        Box::new(self.additional_envs.clone().into_iter())
    }
}
