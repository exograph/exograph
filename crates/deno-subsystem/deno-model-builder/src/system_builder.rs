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

use deno::{module_loader::NpmModuleLoader, CliFactory, CliOptions, Flags, PathBuf};
use deno_ast::{MediaType, ParseParams, SourceTextInfo};
use deno_graph::{Module, ModuleEntryRef, ModuleGraph, ModuleSpecifier, WalkOptions};
use deno_model::{
    interceptor::Interceptor,
    operation::{DenoMutation, DenoQuery},
    subsystem::DenoSubsystem,
};
use deno_runtime::{
    deno_node::{NodeResolution, NodeResolver},
    permissions::PermissionsContainer,
};
use deno_virtual_fs::virtual_fs::{VfsBuilder, VirtualDirectory};
use exo_deno::deno_executor_pool::{DenoScriptDefn, ResolvedModule};
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

    fn run_local<F, R>(future: F) -> R
    where
        F: std::future::Future<Output = R>,
    {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(32)
            .build()
            .unwrap();
        let local = tokio::task::LocalSet::new();
        local.block_on(&rt, future)
    }

    let root = Url::from_file_path(std::fs::canonicalize(module_fs_path).unwrap()).unwrap();
    let root_clone = root.clone();

    // TODO: Make the process_script function async. Currently, we can't because `ProcState` isn't a
    // `Send`. But to make this useful as a callback, we would make this function return a
    // `BoxFuture`, which has the `Send` requirement.
    // TODO: Note that ProcState no longer exists in later versions of deno and has been replaced
    // with CliFactory.
    let script_defn = std::thread::spawn(move || {
        let future = async move {
            let cli_options = CliOptions::new(
                Flags::default(),
                std::env::current_dir().unwrap(),
                None,
                None,
                None,
            )
            .unwrap();
            let factory = CliFactory::from_cli_options(Arc::new(cli_options));
            let module_graph_builder = factory.module_graph_builder().await.map_err(|e| {
                ModelBuildingError::Generic(format!(
                    "While trying to create Deno graph loader: {:?}",
                    e
                ))
            })?;
            let mut loader = module_graph_builder.create_graph_loader();
            let graph = module_graph_builder
                .create_graph_with_loader(
                    deno_graph::GraphKind::CodeOnly,
                    vec![root_clone],
                    &mut loader,
                )
                .await
                .map_err(|e| {
                    ModelBuildingError::Generic(format!(
                        "While trying to create Deno graph: {:?}",
                        e
                    ))
                })?;

            let registry_url = factory.npm_api().unwrap().base_url();
            let root_path = factory.npm_cache().unwrap().registry_folder(registry_url);

            let node_resolver = factory.node_resolver().await.unwrap();
            let npm_resolver = factory.npm_resolver().await.unwrap();
            let npm_resolution = factory.npm_resolution().await.unwrap();

            let npm_loader = NpmModuleLoader::new(
                factory.cjs_resolutions().clone(),
                factory.node_code_translator().await.unwrap().clone(),
                factory.fs().clone(),
                factory.node_resolver().await.unwrap().clone(),
            );

            if !root_path.exists() {
                std::fs::create_dir_all(&root_path).unwrap();
            }

            let vfs = if let Ok(mut builder) = VfsBuilder::new(root_path.clone()) {
                for package in npm_resolution.all_system_packages(&Default::default()) {
                    let folder = npm_resolver
                        .resolve_pkg_folder_from_pkg_id(&package.id)
                        .unwrap();
                    builder.add_dir_recursive(&folder).unwrap();
                }

                builder.set_root_dir_name("EXOGRAPH_NPM_MODULES_SNAPSHOT".to_string());

                builder.into_dir_and_files()
            } else {
                (
                    VirtualDirectory {
                        name: "EXOGRAPH_NPM_MODULES_SNAPSHOT".to_string(),
                        entries: vec![],
                    },
                    vec![],
                )
            };

            Ok::<DenoScriptDefn, ModelBuildingError>(DenoScriptDefn {
                modules: walk_module_graph(graph, npm_loader, node_resolver.clone(), root_path)?,
                npm_snapshot: Some((
                    npm_resolution.serialized_valid_snapshot().into_serialized(),
                    vfs.0,
                    vfs.1,
                )),
            })
        };
        run_local(future)
    })
    .join()
    .unwrap()?;

    Ok((root.to_string(), serde_json::to_vec(&script_defn).unwrap()))
}

fn walk_module_graph(
    graph: ModuleGraph,
    npm_loader: NpmModuleLoader,
    node_resolver: Arc<NodeResolver>,
    root_path: PathBuf,
) -> Result<HashMap<Url, ResolvedModule>, ModelBuildingError> {
    let mut modules: HashMap<Url, ResolvedModule> = HashMap::new();

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
                let (module_source, media_type, final_specifier) = match m {
                    Module::Esm(e) => (
                        e.source.to_string(),
                        MediaType::from_specifier(specifier),
                        specifier.clone(),
                    ),
                    Module::Json(j) => (
                        j.source.to_string(),
                        MediaType::from_specifier(specifier),
                        specifier.clone(),
                    ),
                    Module::Npm(npm) => {
                        // trigger CommonJS detection
                        let _ = npm_loader
                            .resolve_nv_ref(&npm.nv_reference, &PermissionsContainer::allow_all())
                            .unwrap();
                        let resolved = node_resolver
                            .resolve_npm_reference(
                                &npm.nv_reference,
                                deno_runtime::deno_node::NodeResolutionMode::Execution,
                                &PermissionsContainer::allow_all(),
                            )
                            .unwrap()
                            .unwrap();

                        match resolved {
                            NodeResolution::BuiltIn(_) => todo!(),
                            NodeResolution::CommonJs(cjs_specifier) => {
                                let loaded = npm_loader
                                    .load_sync_if_in_npm_package(
                                        &cjs_specifier,
                                        None,
                                        &PermissionsContainer::allow_all(),
                                    )
                                    .unwrap()
                                    .unwrap();

                                // Deno generates a thin ESM wrapper that uses an absolute path
                                let mut root_replaced_specifier: ModuleSpecifier =
                                    ModuleSpecifier::from_file_path(
                                        PathBuf::from("/EXOGRAPH_NPM_MODULES_SNAPSHOT").join(
                                            cjs_specifier
                                                .to_file_path()
                                                .unwrap()
                                                .strip_prefix(root_path.clone())
                                                .unwrap()
                                        )
                                    )
                                    .unwrap();
                                root_replaced_specifier.set_host(Some("localhost")).unwrap();

                                let root_replaced = loaded.code.as_str().replace(
                                    &cjs_specifier
                                        .to_file_path()
                                        .unwrap()
                                        .to_str()
                                        .unwrap()
                                        .replace('\\', "\\\\")
                                        .replace('\'', "\\\'")
                                        .replace('\"', "\\\""),
                                    &root_replaced_specifier
                                        .to_file_path()
                                        .unwrap()
                                        .to_str()
                                        .unwrap()
                                        .replace('\\', "\\\\")
                                        .replace('\'', "\\\'")
                                        .replace('\"', "\\\""),
                                );

                                (
                                    root_replaced,
                                    MediaType::JavaScript,
                                    root_replaced_specifier,
                                )
                            }
                            NodeResolution::Esm(_) => todo!(),
                        }
                    }
                    o => {
                        return Err(ModelBuildingError::Generic(format!(
                            "Unexpected module type {o:?} in Deno graph",
                        )))
                    }
                };

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
                    // TODO(shadaj): fail gracefully here
                    parsed.transpile(&Default::default()).unwrap().text
                } else {
                    module_source
                };

                modules.insert(
                    specifier.clone(),
                    ResolvedModule::Module(transpiled, module_type, final_specifier),
                )
            }
            ModuleEntryRef::Redirect(to) => {
                modules.insert(specifier.clone(), ResolvedModule::Redirect(to.clone()))
            }
            ModuleEntryRef::Err(e) => {
                return Err(ModelBuildingError::ExternalResourceParsing(
                    e.to_string_with_range(),
                ))
            }
        };
    }
    Ok(modules)
}
