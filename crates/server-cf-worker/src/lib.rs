mod init;
mod pg;
mod resolve;

use std::collections::HashMap;

use common::env_const::EXO_INTROSPECTION;
use wasm_bindgen::prelude::*;

use exo_env::Environment;

#[wasm_bindgen]
pub async fn init_and_resolve(
    system_bytes: Vec<u8>,
    raw_request: web_sys::Request,
    env: worker::Env,
) -> Result<web_sys::Response, JsValue> {
    init::init(
        system_bytes,
        Box::new(WorkerEnvironment {
            env,
            additional_envs: HashMap::from([(EXO_INTROSPECTION.to_owned(), "disabled".to_owned())]),
        }),
    )
    .await?;
    resolve::resolve(raw_request).await
}

struct WorkerEnvironment {
    env: worker::Env,
    additional_envs: HashMap<String, String>,
}

impl Environment for WorkerEnvironment {
    fn get(&self, key: &str) -> Option<String> {
        self.env
            .var(key)
            .ok()
            .map(|binding| binding.to_string())
            .or(self.additional_envs.get(key).cloned())
    }
}
