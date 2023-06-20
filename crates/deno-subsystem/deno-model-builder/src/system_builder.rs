// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{collections::HashMap, path::Path, sync::Arc};

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
use deno_ast::{MediaType, ParseParams, SourceTextInfo};
use deno_graph::{Module, ModuleEntryRef, WalkOptions};
use deno_model::{
    interceptor::Interceptor,
    operation::{DenoMutation, DenoQuery},
    subsystem::DenoSubsystem,
};
use exo_deno::deno_executor_pool::ResolvedModule;
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
) -> Result<(String, Vec<u8>), ModelBuildingError> {
    module_skeleton_generator::generate_module_skeleton(module, base_system, module_fs_path)?;

    // TODO: Make the process_script function async. Currently, we can't because `ProcState` isn't a
    // `Send`. But to make this useful as a callback, we would make this function return a
    // `BoxFuture`, which has the `Send` requirement.
    let ps = std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(ProcState::from_options(Arc::new(
            CliOptions::new(
                Flags::default(),
                std::env::current_dir().unwrap(),
                None,
                None,
                None,
            )
            .unwrap(),
        )))
        .unwrap()
    })
    .join()
    .unwrap();

    let mut cache = ps.create_graph_loader();
    let root = Url::from_file_path(std::fs::canonicalize(module_fs_path).unwrap()).unwrap();
    let root_clone = root.clone();
    let graph = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(ps.create_graph_with_loader(vec![root_clone], &mut cache))
            .map_err(|e| {
                ModelBuildingError::Generic(format!("While trying to create Deno graph: {:?}", e))
            })
    })
    .join()
    .unwrap()?;

    let mut modules = HashMap::new();
    for (specifier, maybe_module) in graph.walk(
        &graph.roots,
        WalkOptions {
            follow_dynamic: true,
            follow_type_only: false,
            check_js: true,
        },
    ) {
        match maybe_module {
            ModuleEntryRef::Module(m) => {
                let module_source = match m {
                    Module::Esm(e) => e.source.to_string(),
                    Module::Json(j) => j.source.to_string(),
                    o => {
                        return Err(ModelBuildingError::Generic(format!(
                            "Unexpected module type {o:?} in Deno graph",
                        )))
                    }
                };

                let media_type = MediaType::from(specifier);

                // from Deno examples
                let (module_type, should_transpile) = match media_type {
                    MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
                        (deno_core::ModuleType::JavaScript, false)
                    }
                    MediaType::Jsx => (deno_core::ModuleType::JavaScript, true),
                    MediaType::TypeScript
                    | MediaType::Mts
                    | MediaType::Cts
                    | MediaType::Dts
                    | MediaType::Dmts
                    | MediaType::Dcts
                    | MediaType::Tsx => (deno_core::ModuleType::JavaScript, true),
                    MediaType::Json => (deno_core::ModuleType::Json, false),
                    _ => panic!("Unknown media type {:?}", media_type),
                };

                let transpiled = if should_transpile {
                    let parsed = deno_ast::parse_module(ParseParams {
                        specifier: specifier.to_string(),
                        text_info: SourceTextInfo::from_string(module_source),
                        media_type,
                        capture_tokens: false,
                        scope_analysis: false,
                        maybe_syntax: None,
                    })
                    .unwrap();
                    parsed.transpile(&Default::default()).unwrap().text
                } else {
                    module_source
                };

                modules.insert(
                    specifier,
                    ResolvedModule::Module(transpiled.as_bytes().to_vec(), module_type),
                )
            }
            ModuleEntryRef::Redirect(to) => {
                modules.insert(specifier, ResolvedModule::Redirect(to.clone()))
            }
            ModuleEntryRef::Err(e) => {
                return Err(ModelBuildingError::Generic(format!(
                    "Error in Deno graph: {:?}",
                    e
                )))
            }
        };
    }

    Ok((root.to_string(), bincode::serialize(&modules).unwrap()))
}
