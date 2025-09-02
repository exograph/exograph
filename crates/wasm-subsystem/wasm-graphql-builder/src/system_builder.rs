// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use core_model_builder::{
    ast::ast_types::{AstExpr, AstModule},
    builder::{resolved_builder::AnnotationMapHelper, system_builder::BaseModelSystem},
    error::ModelBuildingError,
    typechecker::{Typed, typ::TypecheckedSystem},
};
use std::path::Path;
use subsystem_model_builder_util::ScriptProcessor;
use wasm_graphql_model::{
    interceptor::Interceptor,
    operation::{WasmMutation, WasmQuery},
    subsystem::WasmSubsystem,
};
pub struct ModelWasmSystemWithInterceptors {
    pub underlying: WasmSubsystem,

    pub interceptors: Vec<(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)>,
}

pub async fn build(
    typechecked_system: &TypecheckedSystem,
    base_system: &BaseModelSystem,
) -> Result<Option<ModelWasmSystemWithInterceptors>, ModelBuildingError> {
    let module_selection_closure =
        |module: &AstModule<Typed>| module.annotations.get("wasm").map(|_| "wasm".to_string());

    let module_system = subsystem_model_builder_util::build_with_selection(
        typechecked_system,
        base_system,
        module_selection_closure,
        WasmScriptProcessor {},
    )
    .await?;

    let underlying_module_system = module_system.underlying;

    if underlying_module_system.queries.is_empty()
        && underlying_module_system.mutations.is_empty()
        && underlying_module_system.interceptors.is_empty()
    {
        return Ok(None);
    }

    let mut queries = MappedArena::default();
    for query in underlying_module_system.queries.values().into_iter() {
        queries.add(&query.name.clone(), WasmQuery(query));
    }

    let mut mutations = MappedArena::default();
    for mutation in underlying_module_system.mutations.values().into_iter() {
        mutations.add(&mutation.name.clone(), WasmMutation(mutation));
    }

    Ok(Some(ModelWasmSystemWithInterceptors {
        underlying: WasmSubsystem {
            contexts: underlying_module_system.contexts,
            module_types: underlying_module_system.module_types,
            queries,
            mutations,
            methods: underlying_module_system.methods,
            scripts: underlying_module_system.scripts,
            interceptors: underlying_module_system.interceptors,
        },
        interceptors: module_system.interceptors,
    }))
}

struct WasmScriptProcessor {}

#[async_trait]
impl ScriptProcessor for WasmScriptProcessor {
    async fn process_script(
        &self,
        _module: &AstModule<Typed>,
        _base_system: &BaseModelSystem,
        _typechecked_system: &TypecheckedSystem,
        module_fs_path: &Path,
    ) -> Result<(String, Vec<u8>), ModelBuildingError> {
        std::fs::read(module_fs_path)
            .map(|o| (module_fs_path.to_str().unwrap().to_string(), o))
            .map_err(|err| {
                ModelBuildingError::Generic(format!("While trying to read bundled module: {err}"))
            })
    }
}
