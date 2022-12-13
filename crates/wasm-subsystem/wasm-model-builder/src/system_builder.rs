use core_plugin_interface::{
    core_model::mapped_arena::{MappedArena, SerializableSlabIndex},
    core_model_builder::{
        ast::ast_types::{AstExpr, AstService},
        builder::{resolved_builder::AnnotationMapHelper, system_builder::BaseModelSystem},
        error::ModelBuildingError,
        typechecker::{typ::TypecheckedSystem, Typed},
    },
};
use std::path::PathBuf;
use wasm_model::{
    interceptor::Interceptor,
    model::ModelWasmSystem,
    operation::{WasmMutation, WasmQuery},
};
pub struct ModelWasmSystemWithInterceptors {
    pub underlying: ModelWasmSystem,

    pub interceptors: Vec<(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)>,
}

pub fn build(
    typechecked_system: &TypecheckedSystem,
    base_system: &BaseModelSystem,
) -> Result<Option<ModelWasmSystemWithInterceptors>, ModelBuildingError> {
    let service_selection_closure =
        |service: &AstService<Typed>| service.annotations.get("wasm").map(|_| "wasm".to_string());

    let service_system = subsystem_model_builder_util::build_with_selection(
        typechecked_system,
        base_system,
        service_selection_closure,
        process_script,
    )?;

    let underlying_service_system = service_system.underlying;

    if underlying_service_system.queries.is_empty()
        && underlying_service_system.mutations.is_empty()
        && underlying_service_system.interceptors.is_empty()
    {
        return Ok(None);
    }

    let mut queries = MappedArena::default();
    for query in underlying_service_system.queries.values.into_iter() {
        queries.add(&query.name.clone(), WasmQuery(query));
    }

    let mut mutations = MappedArena::default();
    for mutation in underlying_service_system.mutations.values.into_iter() {
        mutations.add(&mutation.name.clone(), WasmMutation(mutation));
    }

    Ok(Some(ModelWasmSystemWithInterceptors {
        underlying: ModelWasmSystem {
            contexts: underlying_service_system.contexts,
            service_types: underlying_service_system.service_types,
            queries,
            mutations,
            methods: underlying_service_system.methods,
            scripts: underlying_service_system.scripts,
            interceptors: underlying_service_system.interceptors,
        },
        interceptors: service_system.interceptors,
    }))
}

fn process_script(
    _service: &AstService<Typed>,
    module_fs_path: &PathBuf,
) -> Result<Vec<u8>, ModelBuildingError> {
    std::fs::read(module_fs_path).map_err(|err| {
        ModelBuildingError::Generic(format!(
            "While trying to read bundled service module: {}",
            err
        ))
    })
}
