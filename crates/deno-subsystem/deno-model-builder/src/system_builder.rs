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

use deno::{
    args::{create_default_npmrc, CliOptions},
    cache::{ModuleInfoCache, ParsedSourceCache},
    node::CliCjsCodeAnalyzer,
    npm::ManagedCliNpmResolver,
    resolver::NpmModuleLoader,
    CliFactory, Flags, PathBuf,
};
use deno_ast::{EmitOptions, MediaType, ParseParams};
use deno_graph::{
    DependencyDescriptor, DynamicArgument, Module, ModuleAnalyzer, ModuleEntryRef, ModuleGraph,
    ModuleSpecifier, WalkOptions,
};
use deno_model::{
    interceptor::Interceptor,
    operation::{DenoMutation, DenoQuery},
    subsystem::DenoSubsystem,
};
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_node::{
    analyze::NodeCodeTranslator, NodeResolution, NodeResolutionMode, NodeResolver,
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

    // TODO: Make the process_script function async. Currently, we can't because `CliOptions` isn't a
    // `Send`. But to make this useful as a callback, we would make this function return a
    // `BoxFuture`, which has the `Send` requirement.
    let script_defn = std::thread::spawn(move || {
        let future = async move {
            let cli_options = CliOptions::new(
                Flags::default(),
                std::env::current_dir().unwrap(),
                None,
                None,
                None,
                create_default_npmrc(),
                false,
            )
            .unwrap();
            let factory = CliFactory::from_cli_options(Arc::new(cli_options));
            let module_graph_builder = factory.module_graph_builder().await.map_err(|e| {
                ModelBuildingError::Generic(format!(
                    "While trying to create Deno graph loader: {e:?}"
                ))
            })?;
            let graph = {
                let module_graph_creator = factory.module_graph_creator().await.map_err(|e| {
                    ModelBuildingError::Generic(format!(
                        "While trying to create Deno graph creator: {e:?}"
                    ))
                })?;
                let mut loader = module_graph_builder.create_graph_loader();
                module_graph_creator
                    .create_graph_with_loader(
                        deno_graph::GraphKind::CodeOnly,
                        vec![root_clone],
                        &mut loader,
                    )
                    .await
                    .map_err(|e| {
                        ModelBuildingError::Generic(format!(
                            "While trying to create Deno graph: {e:?}"
                        ))
                    })?
            };

            let node_resolver = factory.node_resolver().await.unwrap();
            let npm_resolver = factory.npm_resolver().await.unwrap();

            let code_translator = factory.node_code_translator().await.unwrap().clone();
            let parsed_source_cache = factory.parsed_source_cache().clone();
            let module_info_cache = factory.module_info_cache().unwrap().clone();
            let npm_loader = NpmModuleLoader::new(
                factory.cjs_resolutions().clone(),
                code_translator.clone(),
                factory.fs().clone(),
                factory.cli_node_resolver().await.unwrap().clone(),
            );

            let managed_npm = npm_resolver.as_managed().unwrap();
            let root_path = npm_resolver
                .as_managed()
                .unwrap()
                .global_cache_root_folder();
            if !root_path.exists() {
                std::fs::create_dir_all(&root_path).unwrap();
            }

            let vfs = if let Ok(mut builder) = VfsBuilder::new(root_path.clone()) {
                for package in managed_npm.all_system_packages(&Default::default()) {
                    let folder = managed_npm
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
                modules: walk_module_graph(
                    graph,
                    npm_loader,
                    node_resolver.clone(),
                    managed_npm,
                    code_translator,
                    parsed_source_cache,
                    module_info_cache,
                    root_path,
                )
                .await?,
                npm_snapshot: Some((
                    managed_npm
                        .serialized_valid_snapshot_for_system(&NpmSystemInfo::default())
                        .into_serialized(),
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

#[allow(clippy::too_many_arguments)]
#[async_recursion::async_recursion(?Send)]
async fn walk_node_resolutions(
    root: NodeResolution,
    npm_loader: &NpmModuleLoader,
    node_resolver: &Arc<NodeResolver>,
    code_translator: &Arc<NodeCodeTranslator<CliCjsCodeAnalyzer>>,
    parsed_source_cache: &Arc<ParsedSourceCache>,
    module_info_cache: &Arc<ModuleInfoCache>,
    root_path: &PathBuf,
    modules: &mut HashMap<Url, ResolvedModule>,
) -> Option<(String, MediaType, ModuleSpecifier, bool)> {
    match root {
        NodeResolution::BuiltIn(_) => None,
        NodeResolution::CommonJs(cjs_specifier) => {
            let loaded = npm_loader
                .load_if_in_npm_package(&cjs_specifier, None)
                .await
                .unwrap()
                .unwrap();

            let loaded_rewritten = code_translator
                .translate_cjs_to_esm(
                    &cjs_specifier,
                    Some(String::from_utf8(loaded.code.as_bytes().to_vec()).unwrap()),
                )
                .await
                .unwrap();

            let cjs_path = cjs_specifier.to_file_path().unwrap();

            let node_relative_path = cjs_path.strip_prefix(root_path.clone()).unwrap();

            // encode the segments of the path with forward slashes, even on windows
            let node_relative_path_str = node_relative_path
                .components()
                .map(|c| c.as_os_str().to_str().unwrap())
                .collect::<Vec<_>>()
                .join("/");

            // Deno generates a thin ESM wrapper that uses an absolute path
            let mut root_replaced_specifier: ModuleSpecifier = ModuleSpecifier::parse(&format!(
                "file:///EXOGRAPH_NPM_MODULES_SNAPSHOT/{node_relative_path_str}"
            ))
            .unwrap();
            root_replaced_specifier.set_host(Some("localhost")).unwrap();

            let root_replaced = loaded_rewritten.as_str().replace(
                &cjs_specifier
                    .to_file_path()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace('\\', "\\\\")
                    .replace('\'', "\\\'")
                    .replace('\"', "\\\""),
                &format!("/EXOGRAPH_NPM_MODULES_SNAPSHOT/{node_relative_path_str}")
                    .replace('\\', "\\\\")
                    .replace('\'', "\\\'")
                    .replace('\"', "\\\""),
            );

            if !modules.contains_key(&root_replaced_specifier) {
                modules.insert(
                    root_replaced_specifier.clone(),
                    ResolvedModule::Module(
                        root_replaced.clone(),
                        deno_core::ModuleType::JavaScript,
                        root_replaced_specifier.clone(),
                        true,
                    ),
                );
            }

            Some((
                root_replaced,
                MediaType::JavaScript,
                root_replaced_specifier,
                true,
            ))
        }
        NodeResolution::Esm(esm_specifier) => {
            let loaded = npm_loader
                .load_if_in_npm_package(&esm_specifier, None)
                .await
                .unwrap()
                .unwrap();

            let esm_path = esm_specifier.to_file_path().unwrap();

            let node_relative_path = esm_path.strip_prefix(root_path.clone()).unwrap();

            // encode the segments of the path with forward slashes, even on windows
            let node_relative_path_str = node_relative_path
                .components()
                .skip(1)
                .map(|c| c.as_os_str().to_str().unwrap())
                .collect::<Vec<_>>()
                .join("/");

            // Deno generates a thin ESM wrapper that uses an absolute path
            let mut root_replaced_specifier: ModuleSpecifier = ModuleSpecifier::parse(&format!(
                "file:///EXOGRAPH_NPM_MODULES_SNAPSHOT/{node_relative_path_str}"
            ))
            .unwrap();
            root_replaced_specifier.set_host(Some("localhost")).unwrap();

            if !modules.contains_key(&root_replaced_specifier) {
                // insert first so that we don't recurse infinitely
                modules.insert(
                    root_replaced_specifier.clone(),
                    ResolvedModule::Module(
                        String::from_utf8(loaded.code.as_bytes().to_vec()).unwrap(),
                        deno_core::ModuleType::JavaScript,
                        root_replaced_specifier.clone(),
                        false,
                    ),
                );

                let analyzer = module_info_cache.as_module_analyzer(parsed_source_cache);

                let analysis = analyzer
                    .analyze(
                        &esm_specifier,
                        Arc::from(String::from_utf8(loaded.code.as_bytes().to_vec()).unwrap()),
                        MediaType::JavaScript,
                    )
                    .await
                    .expect("Failed to analyze dependencies of an ESM NPM module");

                for dep in &analysis.dependencies {
                    let specifier = match dep {
                        DependencyDescriptor::Static(dep) => &dep.specifier,
                        DependencyDescriptor::Dynamic(dep) => match &dep.argument {
                            DynamicArgument::String(s) => s,
                            DynamicArgument::Template(t) => {
                                panic!("Dynamic dependencies with template aren't supported: {t:?}")
                            }
                            DynamicArgument::Expr => {
                                panic!("Dynamic dependencies with expression aren't supported")
                            }
                        },
                    };
                    let resolved = node_resolver
                        .resolve(
                            specifier,
                            &esm_specifier,
                            deno_runtime::deno_node::NodeResolutionMode::Execution,
                        )
                        .expect("Failed to resolve dependency of an ESM NPM module")
                        .expect("Failed to resolve dependency of an ESM NPM module");

                    walk_node_resolutions(
                        resolved,
                        npm_loader,
                        node_resolver,
                        code_translator,
                        parsed_source_cache,
                        module_info_cache,
                        root_path,
                        modules,
                    )
                    .await;
                }
            }

            Some((
                String::from_utf8(loaded.code.as_bytes().to_vec()).unwrap(),
                MediaType::JavaScript,
                root_replaced_specifier,
                false,
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn walk_module_graph(
    graph: ModuleGraph,
    npm_loader: NpmModuleLoader,
    node_resolver: Arc<NodeResolver>,
    npm_resolver: &ManagedCliNpmResolver,
    code_translator: Arc<NodeCodeTranslator<CliCjsCodeAnalyzer>>,
    parsed_source_cache: Arc<ParsedSourceCache>,
    module_info_cache: Arc<ModuleInfoCache>,
    root_path: PathBuf,
) -> Result<HashMap<Url, ResolvedModule>, ModelBuildingError> {
    let mut modules: HashMap<Url, ResolvedModule> = HashMap::new();

    for (specifier, maybe_module) in graph.walk(
        graph.roots.iter(),
        WalkOptions {
            follow_dynamic: true,
            follow_type_only: false,
            check_js: true,
            prefer_fast_check_graph: false,
        },
    ) {
        match maybe_module {
            ModuleEntryRef::Module(m) => {
                let maybe_serializable_module = match m {
                    Module::Js(e) => Some((
                        e.source.to_string(),
                        MediaType::from_specifier(specifier),
                        specifier.clone(),
                        false,
                    )),
                    Module::Json(j) => Some((
                        j.source.to_string(),
                        MediaType::from_specifier(specifier),
                        specifier.clone(),
                        false,
                    )),
                    Module::Node(_) => None,
                    Module::Npm(npm) => {
                        let containing_folder = npm_resolver
                            .resolve_pkg_folder_from_deno_module(npm.nv_reference.nv())
                            .unwrap();
                        let resolved = node_resolver
                            .resolve_package_subpath_from_deno_module(
                                &containing_folder,
                                npm.nv_reference.sub_path(),
                                // this uses the module as its own referrer, but this seems to be fine
                                specifier,
                                NodeResolutionMode::Execution,
                            )
                            .unwrap()
                            .unwrap();

                        walk_node_resolutions(
                            resolved,
                            &npm_loader,
                            &node_resolver,
                            &code_translator,
                            &parsed_source_cache,
                            &module_info_cache,
                            &root_path,
                            &mut modules,
                        )
                        .await
                    }
                    o => {
                        return Err(ModelBuildingError::Generic(format!(
                            "Unexpected module type {o:?} in Deno graph",
                        )))
                    }
                };

                if let Some((module_source, media_type, final_specifier, requires_rewrite)) =
                    maybe_serializable_module
                {
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
                        _ => panic!("Unknown media type {media_type:?}"),
                    };

                    let transpiled = if should_transpile {
                        let parsed = deno_ast::parse_module(ParseParams {
                            specifier: specifier.clone(),
                            text: module_source.into(),
                            media_type,
                            capture_tokens: false,
                            scope_analysis: false,
                            maybe_syntax: None,
                        })
                        .unwrap();
                        // TODO(shadaj): fail gracefully here
                        parsed
                            .transpile(&Default::default(), &EmitOptions::default())
                            .unwrap()
                            .into_source()
                            .into_string()
                            .unwrap()
                            .text
                    } else {
                        module_source
                    };

                    modules.insert(
                        specifier.clone(),
                        ResolvedModule::Module(
                            transpiled,
                            module_type,
                            final_specifier,
                            requires_rewrite,
                        ),
                    );
                }
            }
            ModuleEntryRef::Redirect(to) => {
                modules.insert(specifier.clone(), ResolvedModule::Redirect(to.clone()));
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
