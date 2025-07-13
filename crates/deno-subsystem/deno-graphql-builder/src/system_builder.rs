// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{collections::HashMap, env, path::Path};

use async_trait::async_trait;
use common::download::{download_if_needed, exo_cache_root};
use core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use core_model_builder::{
    ast::ast_types::{AstExpr, AstModule},
    builder::{resolved_builder::AnnotationMapHelper, system_builder::BaseModelSystem},
    error::ModelBuildingError,
    plugin::BuildMode,
    typechecker::{Typed, typ::TypecheckedSystem},
};

use deno_core::ModuleType;
use deno_graphql_model::{
    interceptor::Interceptor,
    operation::{DenoMutation, DenoQuery},
    subsystem::DenoSubsystem,
};
use exo_deno::deno_executor_pool::{DenoScriptDefn, ResolvedModule};
use subsystem_model_builder_util::ScriptProcessor;
use url::Url;

use crate::module_skeleton_generator;

const DENO_VERSION: &str = "2.4.1";

async fn bundle_source(module_fs_path: &Path) -> Result<String, ModelBuildingError> {
    let deno_path = exo_cache_root()
        .map_err(|e| {
            ModelBuildingError::Generic(format!("Failed to determine cache root directory: {}", e))
        })?
        .join("deno")
        .join(DENO_VERSION)
        .join("deno");

    if !deno_path.exists() {
        let target_os = env::consts::OS;
        let target_arch = env::consts::ARCH;

        let platform = match (target_os, target_arch) {
            ("macos", "x86_64") => "x86_64-apple-darwin",
            ("macos", "aarch64") => "aarch64-apple-darwin",
            ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
            ("windows", "x86_64") => "x86_64-pc-windows-msvc",
            (os, arch) => {
                return Err(ModelBuildingError::Generic(format!(
                    "Unsupported platform: {os}-{arch}"
                )));
            }
        };

        download_if_needed(
            &format!(
                "https://github.com/denoland/deno/releases/download/v{DENO_VERSION}/deno-{platform}.zip"
            ),
            "Deno",
            Some(&format!("deno/{DENO_VERSION}")),
            true,
        )
        .await
        .map_err(|e| ModelBuildingError::Generic(format!("Failed to download Deno: {}", e)))?;
    }

    let output = std::process::Command::new(deno_path)
        .arg("bundle")
        .arg("--allow-import")
        .arg(module_fs_path.to_string_lossy().as_ref())
        .output()
        .map_err(|e| ModelBuildingError::Generic(format!("Error: {}", e)))?;

    String::from_utf8(output.stdout).map_err(|e| {
        ModelBuildingError::Generic(format!("Failed to parse bundled output as UTF-8: {}", e))
    })
}

pub struct ModelDenoSystemWithInterceptors {
    pub underlying: DenoSubsystem,

    pub interceptors: Vec<(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)>,
}

pub struct DenoScriptProcessor {
    build_mode: BuildMode,
}

#[async_trait]
impl ScriptProcessor for DenoScriptProcessor {
    async fn process_script(
        &self,
        module: &AstModule<Typed>,
        base_system: &BaseModelSystem,
        typechecked_system: &TypecheckedSystem,
        module_fs_path: &Path,
    ) -> Result<(String, Vec<u8>), ModelBuildingError> {
        if self.build_mode == BuildMode::Build {
            module_skeleton_generator::generate_module_skeleton(
                module,
                base_system,
                typechecked_system,
                module_fs_path,
            )?;
        }

        let root = Url::from_file_path(std::fs::canonicalize(module_fs_path).unwrap()).unwrap();

        let bundled = bundle_source(module_fs_path).await?;

        let script_defn = DenoScriptDefn {
            modules: HashMap::from([(
                root.clone(),
                ResolvedModule::Module(bundled, ModuleType::JavaScript, root.clone(), false),
            )]),
        };

        Ok((root.to_string(), serde_json::to_vec(&script_defn).unwrap()))
    }
}

pub async fn build(
    typechecked_system: &TypecheckedSystem,
    base_system: &BaseModelSystem,
    build_mode: BuildMode,
) -> Result<Option<ModelDenoSystemWithInterceptors>, ModelBuildingError> {
    let module_selection_closure =
        |module: &AstModule<Typed>| module.annotations.get("deno").map(|_| "deno".to_string());

    let script_processor = DenoScriptProcessor { build_mode };

    let module_system = subsystem_model_builder_util::build_with_selection(
        typechecked_system,
        base_system,
        module_selection_closure,
        script_processor,
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
