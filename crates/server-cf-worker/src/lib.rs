mod init;
mod pg;
mod resolve;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn init_and_resolve(
    system_bytes: Vec<u8>,
    raw_request: web_sys::Request,
    env: worker::Env,
) -> Result<web_sys::Response, JsValue> {
    init::init(env, system_bytes).await?;
    resolve::resolve(raw_request).await
}
