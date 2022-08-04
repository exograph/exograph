use deno_core::Extension;
use tokio::sync::oneshot;

use async_trait::async_trait;
use futures::future::BoxFuture;
use serde_json::Value;

use payas_deno::{
    deno_executor::CallbackProcessor,
    deno_executor_pool::DenoExecutorConfig,
    deno_module::{DenoModule, DenoModuleSharedState},
};
use payas_resolver_core::QueryResponse;

use super::{claytip_ops::InterceptedOperationInfo, DenoExecutionError};

#[derive(Default, Debug)]
pub struct ClaytipMethodResponse {
    pub(crate) headers: Vec<(String, String)>,
}

pub enum RequestFromDenoMessage {
    InterceptedOperationProceed {
        response_sender: oneshot::Sender<ResponseForDenoMessage>,
    },
    ClaytipExecute {
        query_string: String,
        variables: Option<serde_json::Map<String, Value>>,
        context_override: Value,
        response_sender: oneshot::Sender<ResponseForDenoMessage>,
    },
}

pub enum ResponseForDenoMessage {
    InterceptedOperationProceed(Result<QueryResponse, DenoExecutionError>),
    ClaytipExecute(Result<QueryResponse, DenoExecutionError>),
}

pub type FnClaytipExecuteQuery<'a> = (dyn Fn(
    String,
    Option<serde_json::Map<String, Value>>,
    Value,
) -> BoxFuture<'a, Result<QueryResponse, DenoExecutionError>>
     + 'a
     + Send
     + Sync);
pub type FnClaytipInterceptorProceed<'a> =
    (dyn Fn() -> BoxFuture<'a, Result<QueryResponse, DenoExecutionError>> + 'a + Send + Sync);

pub struct ClayCallbackProcessor<'a> {
    pub claytip_execute_query: &'a FnClaytipExecuteQuery<'a>,
    pub claytip_proceed: Option<&'a FnClaytipInterceptorProceed<'a>>,
}

#[async_trait]
impl<'a> CallbackProcessor<RequestFromDenoMessage> for ClayCallbackProcessor<'a> {
    async fn process_callback(&self, req: RequestFromDenoMessage) {
        match req {
            RequestFromDenoMessage::InterceptedOperationProceed { response_sender } => {
                let proceed_result = self.claytip_proceed.unwrap()().await;
                response_sender
                    .send(ResponseForDenoMessage::InterceptedOperationProceed(
                        proceed_result,
                    ))
                    .ok()
                    .unwrap();
            }
            RequestFromDenoMessage::ClaytipExecute {
                query_string,
                variables,
                context_override,
                response_sender,
            } => {
                let query_result =
                    (self.claytip_execute_query)(query_string, variables, context_override).await;
                response_sender
                    .send(ResponseForDenoMessage::ClaytipExecute(query_result))
                    .ok()
                    .unwrap();
            }
        }
    }
}

const SHIMS: [(&str, &[&str]); 3] = {
    let claytip_shim = include_str!("claytip_shim.js");
    [
        ("Claytip", &[claytip_shim]),
        (
            "ClaytipPriv",
            // Pass both the shim and the private shim so that in effect we get `ClaytipPriv extends Claytip`.
            &[claytip_shim, include_str!("claytip_priv_shim.js")],
        ),
        ("Operation", &[include_str!("operation_shim.js")]),
    ]
};

const USER_AGENT: &str = "Claytip";
const ADDITIONAL_CODE: &[&str] = &[include_str!("./claytip_error.js")];
const EXPLICIT_ERROR_CLASS_NAME: Option<&'static str> = Some("ClaytipError");

pub fn process_call_context(
    deno_module: &mut DenoModule,
    call_context: Option<InterceptedOperationInfo>,
) {
    deno_module
        .put(call_context)
        .unwrap_or_else(|_| panic!("Failed to setup interceptor"));
}

pub fn clay_config() -> DenoExecutorConfig<Option<InterceptedOperationInfo>> {
    fn create_extensions() -> Vec<Extension> {
        // we provide a set of Claytip functionality through custom Deno ops,
        // create a Deno extension that provides these ops
        let ext = Extension::builder()
            .ops(vec![
                super::claytip_ops::op_claytip_execute_query::decl(),
                super::claytip_ops::op_claytip_execute_query_priv::decl(),
                super::claytip_ops::op_intercepted_operation_name::decl(),
                super::claytip_ops::op_intercepted_operation_query::decl(),
                super::claytip_ops::op_intercepted_proceed::decl(),
                super::claytip_ops::op_add_header::decl(),
            ])
            .build();
        vec![ext]
    }

    DenoExecutorConfig::new(
        USER_AGENT,
        SHIMS.to_vec(),
        ADDITIONAL_CODE.to_vec(),
        EXPLICIT_ERROR_CLASS_NAME,
        create_extensions,
        process_call_context,
        DenoModuleSharedState::default(),
    )
}
