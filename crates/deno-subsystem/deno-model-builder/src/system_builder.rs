// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{path::Path, sync::Arc};

use core_plugin_interface::{
    core_model::mapped_arena::{MappedArena, SerializableSlabIndex},
    core_model_builder::{
        ast::ast_types::{AstExpr, AstModule},
        builder::{resolved_builder::AnnotationMapHelper, system_builder::BaseModelSystem},
        error::ModelBuildingError,
        typechecker::{typ::TypecheckedSystem, Typed},
    },
};

use deno::{CliOptions, Flags, ProcState};
use deno_model::{
    interceptor::Interceptor,
    operation::{DenoMutation, DenoQuery},
    subsystem::DenoSubsystem,
};
use url::Url;

use crate::module_skeleton_generator;

pub struct ModelDenoSystemWithInterceptors {
    pub underlying: DenoSubsystem,

    pub interceptors: Vec<(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)>,
}

pub async fn build(
    typechecked_system: &TypecheckedSystem,
    base_system: &BaseModelSystem,
) -> Result<Option<ModelDenoSystemWithInterceptors>, ModelBuildingError> {
    let module_selection_closure =
        |module: &AstModule<Typed>| module.annotations.get("deno").map(|_| "deno".to_string());

    let module_system = subsystem_model_builder_util::build_with_selection(
        typechecked_system,
        base_system,
        module_selection_closure,
        process_script,
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
        queries.add(&query.name.clone(), DenoQuery(query));
    }

    let mut mutations = MappedArena::default();
    for mutation in underlying_module_system.mutations.values().into_iter() {
        mutations.add(&mutation.name.clone(), DenoMutation(mutation));
    }

    Ok(Some(ModelDenoSystemWithInterceptors {
        underlying: DenoSubsystem {
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

fn process_script(
    module: &AstModule<Typed>,
    base_system: &BaseModelSystem,
    module_fs_path: &Path,
) -> Result<Vec<u8>, ModelBuildingError> {
    module_skeleton_generator::generate_module_skeleton(module, base_system, module_fs_path)?;
    let tokio_runtime = tokio::runtime::Runtime::new().unwrap();

    let ps = tokio_runtime
        .block_on(ProcState::from_options(Arc::new(
            CliOptions::new(
                Flags::default(),
                std::env::current_dir().unwrap(),
                None,
                None,
                None,
            )
            .unwrap(),
        )))
        .unwrap();

    let path = format!(
        "file://{}",
        std::fs::canonicalize(module_fs_path)
            .unwrap()
            .to_str()
            .unwrap()
    );

    let mut cache = ps.create_graph_loader();
    let graph = tokio_runtime
        .block_on(ps.create_graph_with_loader(vec![Url::parse(&path).unwrap()], &mut cache))
        .map_err(|e| {
            ModelBuildingError::Generic(format!("While trying to create Deno graph: {:?}", e))
        })?;

    let bundle_res = deno_emit::bundle_graph(
        &graph,
        deno_emit::BundleOptions {
            bundle_type: deno_emit::BundleType::Module,
            emit_ignore_directives: false,
            emit_options: deno_emit::EmitOptions::default(),
        },
    )
    .map_err(|e| {
        ModelBuildingError::Generic(format!("While trying to bundle Deno script: {:?}", e))
    })?;

    Ok(bundle_res.code.into_bytes())
}
