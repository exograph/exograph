mod env;
mod init;
mod pg;
mod resolve;

use std::collections::HashMap;

use common::env_const::EXO_INTROSPECTION;
use env::WorkerEnvironment;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn init_and_resolve(
    system_bytes: Vec<u8>,
    raw_request: web_sys::Request,
    env: worker::Env,
) -> Result<web_sys::Response, JsValue> {
    init::init(
        system_bytes,
        WorkerEnvironment::new(
            env,
            HashMap::from([(EXO_INTROSPECTION.to_owned(), "disabled".to_owned())]),
        ),
    )
    .await?;
    resolve::resolve(raw_request).await
}
