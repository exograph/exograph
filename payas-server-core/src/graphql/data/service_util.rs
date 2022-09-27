use payas_core_model::mapped_arena::SerializableSlabIndex;
use payas_deno_model::{
    model::ModelDenoSystem,
    service::{ScriptKind, ServiceMethod},
};
use payas_resolver_core::{request_context::RequestContext, validation::field::ValidatedField};
use payas_resolver_deno::DenoOperation;
use payas_resolver_wasm::WasmOperation;
use payas_wasm_model::model::ModelWasmSystem;

use crate::graphql::execution_error::ExecutionError;

use super::data_operation::DataOperation;

pub(crate) fn create_deno_operation<'a>(
    system: &'a ModelDenoSystem,
    method_id: &Option<SerializableSlabIndex<ServiceMethod>>,
    field: &'a ValidatedField,
    request_context: &'a RequestContext<'a>,
) -> Result<DataOperation<'a>, ExecutionError> {
    // TODO: Remove unwrap() by changing the type of method_id
    let method = &system.methods[method_id.unwrap()];
    let script = &system.scripts[method.script];

    Ok(match script.script_kind {
        ScriptKind::Deno => DataOperation::Deno(DenoOperation {
            method,
            field,
            request_context,
        }),
        ScriptKind::Wasm => DataOperation::Wasm(WasmOperation {
            method,
            field,
            request_context,
        }),
    })
}

pub(crate) fn create_wasm_operation<'a>(
    system: &'a ModelWasmSystem,
    method_id: &Option<SerializableSlabIndex<ServiceMethod>>,
    field: &'a ValidatedField,
    request_context: &'a RequestContext<'a>,
) -> Result<DataOperation<'a>, ExecutionError> {
    // TODO: Remove unwrap() by changing the type of method_id
    let method = &system.methods[method_id.unwrap()];
    let script = &system.scripts[method.script];

    Ok(match script.script_kind {
        ScriptKind::Deno => DataOperation::Deno(DenoOperation {
            method,
            field,
            request_context,
        }),
        ScriptKind::Wasm => DataOperation::Wasm(WasmOperation {
            method,
            field,
            request_context,
        }),
    })
}
