use std::sync::Arc;

use async_trait::async_trait;

use http::StatusCode;

use common::{
    context::RequestContext,
    env_const::get_rpc_http_path,
    http::{Headers, RequestHead, RequestPayload, ResponseBody, ResponsePayload},
    router::Router,
};
use core_resolver::system_rpc_resolver::SystemRpcResolver;
use exo_env::Environment;

pub struct RpcRouter {
    system_resolver: SystemRpcResolver,
    api_path_prefix: String,
}

impl RpcRouter {
    pub fn new(system_resolver: SystemRpcResolver, env: Arc<dyn Environment>) -> Self {
        Self {
            system_resolver,
            api_path_prefix: get_rpc_http_path(env.as_ref()).clone(),
        }
    }

    fn suitable(&self, request_head: &(dyn RequestHead + Sync)) -> bool {
        request_head.get_path() == self.api_path_prefix
    }
}

#[async_trait]
impl<'a> Router<RequestContext<'a>> for RpcRouter {
    async fn route(&self, request_context: &RequestContext<'a>) -> Option<ResponsePayload> {
        if !self.suitable(request_context.get_head()) {
            return None;
        }

        match self.system_resolver.resolve(request_context).await {
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
