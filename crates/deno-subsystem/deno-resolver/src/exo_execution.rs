use deno_core::Extension;
use tokio::sync::oneshot;

use async_trait::async_trait;
use futures::future::BoxFuture;
use serde_json::Value;

use core_plugin_interface::core_resolver::{
    system_resolver::{ExographExecuteQueryFn, SystemResolutionError},
    QueryResponse,
};

use exo_deno::{
    deno_executor::CallbackProcessor,
    deno_executor_pool::DenoExecutorConfig,
    deno_module::{DenoModule, DenoModuleSharedState},
};

use super::exograph_ops::InterceptedOperationInfo;

#[derive(Default, Debug)]
pub struct ExographMethodResponse {
    pub headers: Vec<(String, String)>,
}

pub enum RequestFromDenoMessage {
    InterceptedOperationProceed {
        response_sender: oneshot::Sender<ResponseForDenoMessage>,
    },
    ExographExecute {
        query_string: String,
        variables: Option<serde_json::Map<String, Value>>,
        context_override: Value,
        response_sender: oneshot::Sender<ResponseForDenoMessage>,
    },
}

pub enum ResponseForDenoMessage {
    InterceptedOperationProceed(Result<QueryResponse, SystemResolutionError>),
    ExographExecute(Result<QueryResponse, SystemResolutionError>),
}

pub type FnExographInterceptorProceed<'a> =
    (dyn Fn() -> BoxFuture<'a, Result<QueryResponse, SystemResolutionError>> + 'a + Send + Sync);

pub struct ExoCallbackProcessor<'a, 'b> {
    pub exograph_execute_query: &'a ExographExecuteQueryFn<'a>,
    pub exograph_proceed: Option<&'b FnExographInterceptorProceed<'a>>,
}

#[async_trait]
impl<'a> CallbackProcessor<RequestFromDenoMessage> for ExoCallbackProcessor<'a, '_> {
    async fn process_callback(&self, req: RequestFromDenoMessage) {
        match req {
            RequestFromDenoMessage::InterceptedOperationProceed { response_sender } => {
                let proceed_result = self.exograph_proceed.unwrap()().await;
                response_sender
                    .send(ResponseForDenoMessage::InterceptedOperationProceed(
                        proceed_result,
                    ))
                    .ok()
                    .unwrap();
            }
            RequestFromDenoMessage::ExographExecute {
                query_string,
                variables,
                context_override,
                response_sender,
            } => {
                let query_result =
                    (self.exograph_execute_query)(query_string, variables, context_override).await;
                response_sender
                    .send(ResponseForDenoMessage::ExographExecute(query_result))
                    .ok()
                    .unwrap();
            }
        }
    }
}

const SHIMS: [(&str, &[&str]); 3] = {
    let exograph_shim = include_str!("exograph_shim.js");
    [
        ("Exograph", &[exograph_shim]),
        (
            "ExographPriv",
            // Pass both the shim and the private shim so that in effect we get `ExographPriv extends Exograph`.
            &[exograph_shim, include_str!("exograph_priv_shim.js")],
        ),
        ("Operation", &[include_str!("operation_shim.js")]),
    ]
};

const USER_AGENT: &str = "Exograph";
const ADDITIONAL_CODE: &[&str] = &[include_str!("./exograph_error.js")];
const EXPLICIT_ERROR_CLASS_NAME: Option<&'static str> = Some("ExographError");

pub fn process_call_context(
    deno_module: &mut DenoModule,
    call_context: Option<InterceptedOperationInfo>,
) {
    deno_module
        .put(call_context)
        .unwrap_or_else(|_| panic!("Failed to setup interceptor"));
}

pub fn exo_config() -> DenoExecutorConfig<Option<InterceptedOperationInfo>> {
    fn create_extensions() -> Vec<Extension> {
        // we provide a set of Exograph functionality through custom Deno ops,
        // create a Deno extension that provides these ops
        let ext = Extension::builder("exograph")
            .ops(vec![
                super::exograph_ops::op_exograph_execute_query::decl(),
                super::exograph_ops::op_exograph_execute_query_priv::decl(),
                super::exograph_ops::op_exograph_add_header::decl(),
                super::exograph_ops::op_operation_name::decl(),
                super::exograph_ops::op_operation_query::decl(),
                super::exograph_ops::op_operation_proceed::decl(),
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
