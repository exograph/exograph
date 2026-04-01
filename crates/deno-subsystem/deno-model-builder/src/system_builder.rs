// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use common::download::{download_dir_if_needed, exo_cache_root};
use core_model_builder::{
    ast::ast_types::AstModule,
    builder::{resolved_builder::AnnotationMapHelper, system_builder::BaseModelSystem},
    error::ModelBuildingError,
    plugin::BuildMode,
    typechecker::{Typed, typ::TypecheckedSystem},
};

use deno_core::ModuleType;
use exo_deno::deno_executor_pool::{DenoScriptDefn, ResolvedModule};
use subsystem_model_builder_util::{ModuleSubsystemWithInterceptors, ScriptProcessor};
use url::Url;

use crate::module_skeleton_generator;

// SYNC: update when deno_runtime/deno_core crate versions change in Cargo.toml.
// Find the matching CLI version at https://github.com/denoland/deno/releases
const DENO_VERSION: &str = "2.7.10";

const DENO_BUNDLE_WARNING: &[u8] = b"is experimental and subject to changes";

async fn bundle_source(module_fs_path: &Path) -> Result<String, ModelBuildingError> {
    let deno_path = download_deno_if_needed().await?;

    // Each module gets its own lock file (e.g. `src/foo.tsx.deno.lock`) so that
    // multiple @deno modules in the same project don't overwrite each other.
    let lock_path = PathBuf::from(format!("{}.deno.lock", module_fs_path.to_string_lossy()));

    let output = tokio::process::Command::new(deno_path)
        .arg("bundle")
        .arg("--allow-import")
        .arg("--quiet")
        .arg("--node-modules-dir=auto")
        .arg(format!("--lock={}", lock_path.to_string_lossy()))
        .arg(module_fs_path.to_string_lossy().to_string())
        .output()
        .await;

    fn simplify_error(output: &[u8]) -> String {
        // remove the "experimental" warning by looking for DENO_BUNDLE_WARNING and stripping that out
        let output = output
            .split(|b| *b == b'\n')
            .filter(|line| !line.ends_with(DENO_BUNDLE_WARNING))
            .collect::<Vec<_>>()
            .join(&b'\n');

        let output_str = String::from_utf8_lossy(&output).to_string();

        // Deno bundle output shows the full path to source file, so we drop the current directory portions
        let current_dir_url = Url::from_directory_path(
            std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap(),
        )
        .unwrap()
        .to_string();

        output_str.replace(&current_dir_url, "")
    }

    match output {
        Ok(output) => {
            if !output.status.success() {
                Err(ModelBuildingError::TSJSParsingError(simplify_error(
                    &output.stderr,
                )))
            } else {
                String::from_utf8(output.stdout).map_err(|e| {
                    ModelBuildingError::Generic(format!(
                        "Failed to parse bundled output as UTF-8: {}",
                        e
                    ))
                })
            }
        }
        Err(e) => Err(ModelBuildingError::Generic(format!(
            "Failed to execute Deno: {}",
            e
        ))),
    }
}

async fn download_deno_if_needed() -> Result<PathBuf, ModelBuildingError> {
    let deno_executable = if env::consts::OS == "windows" {
        "deno.exe"
    } else {
        "deno"
    };
    let deno_path = exo_cache_root()
        .map_err(|e| {
            ModelBuildingError::Generic(format!("Failed to determine cache root directory: {}", e))
        })?
        .join("deno")
        .join(DENO_VERSION)
        .join(deno_executable);

    let target_os = env::consts::OS;
    let target_arch = env::consts::ARCH;

    let platform = match (target_os, target_arch) {
        ("macos", "x86_64") => {
            return Err(ModelBuildingError::Generic(
                "Intel Macs (x86_64) are no longer supported.".to_string(),
            ));
        }
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        (os, arch) => {
            return Err(ModelBuildingError::Generic(format!(
                "Unsupported platform: {os}-{arch}"
            )));
        }
    };

    download_dir_if_needed(
        &format!(
            "https://github.com/denoland/deno/releases/download/v{DENO_VERSION}/deno-{platform}.zip"
        ),
        "Deno",
        &format!("deno/{DENO_VERSION}"),
    )
    .await
    .map_err(|e| ModelBuildingError::Generic(format!("Failed to download Deno: {}", e)))?;

    if !deno_path.exists() {
        return Err(ModelBuildingError::Generic(
            "Deno executable not found after download".to_string(),
        ));
    }

    Ok(deno_path)
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

        // Only @deno modules have script files to run.
        if module.annotations.get("deno").is_none() {
            return Ok((String::new(), vec![]));
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

/// Build the Deno module subsystem, returning the shared `ModuleSubsystem` and interceptors.
///
/// This is the protocol-agnostic build step — the result can be used for both
/// GraphQL (by wrapping into `DenoSubsystem`) and RPC (by serializing as `ModuleSubsystem`).
pub async fn build(
    typechecked_system: &TypecheckedSystem,
    base_system: &BaseModelSystem,
    build_mode: BuildMode,
) -> Result<Option<ModuleSubsystemWithInterceptors>, ModelBuildingError> {
    let module_selection_closure = |module: &AstModule<Typed>| {
        ["deno", "postgres"]
            .into_iter()
            .find(|&name| module.annotations.get(name).is_some())
            .map(String::from)
    };

    let script_processor = DenoScriptProcessor { build_mode };

    let module_system = subsystem_model_builder_util::build_with_selection(
        typechecked_system,
        base_system,
        module_selection_closure,
        script_processor,
    )
    .await?;

    let underlying = &module_system.underlying;

    if underlying.queries.is_empty()
        && underlying.mutations.is_empty()
        && underlying.interceptors.is_empty()
    {
        return Ok(None);
    }

    Ok(Some(module_system))
}
