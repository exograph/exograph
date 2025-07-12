// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use deno_core::Extension;
use tokio::sync::oneshot;

use async_trait::async_trait;
use futures::future::BoxFuture;
use serde_json::Value;

use core_plugin_shared::trusted_documents::TrustedDocumentEnforcement;
use core_resolver::{
    QueryResponse,
    system_resolver::{ExographExecuteQueryFn, SystemResolutionError},
};

use exo_deno::{
    deno_executor::CallbackProcessor, deno_executor_pool::DenoExecutorConfig,
    deno_module::DenoModule,
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
impl CallbackProcessor<RequestFromDenoMessage> for ExoCallbackProcessor<'_, '_> {
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
                let query_result = (self.exograph_execute_query)(
                    query_string,
                    variables,
                    TrustedDocumentEnforcement::DoNotEnforce,
                    context_override,
                )
                .await;
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

// We provide a set of Exograph functionality accessible via this Deno extension
deno_core::extension!(
    exograph,
    ops = [
        super::exograph_ops::op_exograph_execute_query,
        super::exograph_ops::op_exograph_execute_query_priv,
        super::exograph_ops::op_exograph_add_header,
        super::exograph_ops::op_exograph_version,
        super::exograph_ops::op_operation_name,
        super::exograph_ops::op_operation_query,
        super::exograph_ops::op_operation_proceed,
    ],
    esm_entry_point = "ext:exograph/__init.js",
    esm = [
        dir "extension",
        "__init.js",
         "exograph:ops.js" = "exograph.js",
    ]
);

pub fn exo_config() -> DenoExecutorConfig<Option<InterceptedOperationInfo>> {
    fn create_extensions() -> Vec<Extension> {
        vec![exograph::init()]
    }

    DenoExecutorConfig::new(
        SHIMS.to_vec(),
        ADDITIONAL_CODE.to_vec(),
        EXPLICIT_ERROR_CLASS_NAME,
        create_extensions,
        process_call_context,
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use test_log::test;

    use super::exograph;
    use exo_deno::{DenoModule, UserCode};

    #[test]
    #[allow(deprecated)]
    fn check_extension_esm_is_embedded() {
        let extension = exograph::init();
        extension.esm_files.iter().for_each(|esm_file| {
            assert!(matches!(
                esm_file.code,
                deno_core::ExtensionFileSourceCode::IncludedInBinary(_)
            ));
        });
    }

    #[test(tokio::test)]
    async fn test_call_version_op() {
        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(
                Path::new("src")
                    .join("test_js")
                    .join("test_exograph_extension.js")
                    .to_owned(),
            ),
            vec![],
            vec![],
            vec![exograph::init()],
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let ret_value = deno_module
            .execute_function("exographVersion", vec![])
            .await
            .unwrap();
        assert_eq!(
            ret_value,
            serde_json::Value::String(env!("CARGO_PKG_VERSION").into())
        );
    }
}
