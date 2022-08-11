use payas_model::model::{
    mapped_arena::SerializableSlabIndex,
    service::{ScriptKind, ServiceMethod},
    system::ModelSystem,
};
use payas_resolver_core::{request_context::RequestContext, validation::field::ValidatedField};
use payas_resolver_deno::DenoOperation;
use payas_resolver_wasm::WasmOperation;

use crate::graphql::execution_error::ExecutionError;

use super::data_operation::DataOperation;

pub(crate) fn create_service_operation<'a>(
    system: &'a ModelSystem,
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
