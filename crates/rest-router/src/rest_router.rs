use std::sync::Arc;

use async_trait::async_trait;

use http::StatusCode;

use common::{
    env_const::get_rest_http_path,
    http::{Headers, RequestHead, RequestPayload, ResponseBody, ResponsePayload},
    router::Router,
};
use core_resolver::system_rest_resolver::SystemRestResolver;
use exo_env::Environment;

pub struct RestRouter {
    system_resolver: SystemRestResolver,
    api_path_prefix: String,
}

impl RestRouter {
    pub fn new(system_resolver: SystemRestResolver, env: Arc<dyn Environment>) -> Self {
        // Set the prefix to "/api" + "/" to avoid matching with routes such as "/apis" etc.
        let api_path_prefix = format!("{}/", get_rest_http_path(env.as_ref()));
        Self {
            system_resolver,
            api_path_prefix,
        }
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

        match self.system_resolver.resolve(request).await {
            Ok(Some(response)) => Some(response),
            Err(e) => {
                tracing::error!("Error resolving subsystem: {}", e);
                Some(ResponsePayload {
                    body: ResponseBody::None,
                    headers: Headers::new(),
                    status_code: StatusCode::INTERNAL_SERVER_ERROR,
                })
            }
            _ => None,
        }
    }
}
