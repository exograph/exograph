use std::sync::Arc;

use async_trait::async_trait;

use common::{
    env_const::get_rest_http_path,
    http::{RequestHead, RequestPayload, ResponsePayload},
    router::Router,
};
use exo_env::Environment;

pub struct RestRouter {
    api_path_prefix: String,
}

impl RestRouter {
    pub fn new(env: Arc<dyn Environment>) -> Self {
        // Set the prefix to "/api" + "/" to avoid matching with routes such as "/apis" etc.
        let api_path_prefix = format!("{}/", get_rest_http_path(env.as_ref()));
        Self { api_path_prefix }
    }

    fn suitable(&self, request_head: &(dyn RequestHead + Sync)) -> bool {
        request_head.get_path().starts_with(&self.api_path_prefix)
    }
}

#[async_trait]
impl Router for RestRouter {
    async fn route(&self, request: &mut (dyn RequestPayload + Send)) -> Option<ResponsePayload> {
        if !self.suitable(request.get_head()) {
            return None;
        }

        todo!()
    }
}
