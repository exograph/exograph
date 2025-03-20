use common::{
    context::RequestContext,
    http::{RequestHead, RequestPayload, ResponsePayload},
    router::{PlainRequestPayload, Router},
};

use core_model::context_type::ContextSelection;
use exo_env::Environment;

use serde_json::Value;

pub(super) struct TestRequest {}

impl RequestPayload for TestRequest {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        self
    }

    fn take_body(&self) -> serde_json::Value {
        Default::default()
    }
}

impl RequestHead for TestRequest {
    fn get_headers(&self, _key: &str) -> Vec<String> {
        vec![]
    }

    fn get_ip(&self) -> Option<std::net::IpAddr> {
        None
    }

    fn get_method(&self) -> http::Method {
        http::Method::POST
    }

    fn get_path(&self) -> String {
        "".to_string()
    }

    fn get_query(&self) -> serde_json::Value {
        Default::default()
    }
}

pub(super) struct TestRouter {}

#[async_trait::async_trait]
impl<'request> Router<PlainRequestPayload<'request>> for TestRouter {
    async fn route(
        &self,
        _request_context: &PlainRequestPayload<'request>,
    ) -> Option<ResponsePayload> {
        None
    }
}

const REQUEST: TestRequest = TestRequest {};

pub(super) fn test_request_context<'a>(
    test_values: Value,
    system_router: &'a (dyn for<'request> Router<PlainRequestPayload<'request>> + Send + Sync),
    env: &'a dyn Environment,
) -> RequestContext<'a> {
    RequestContext::new(
        &REQUEST,
        vec![Box::new(common::context::TestRequestContext {
            test_values,
        })],
        system_router,
        &None,
        env,
    )
}

pub(super) fn context_selection(context_name: &str, path_head: &str) -> ContextSelection {
    ContextSelection {
        context_name: context_name.to_string(),
        path: (path_head.to_string(), vec![]),
    }
}
