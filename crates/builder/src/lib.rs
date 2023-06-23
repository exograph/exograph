// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{env::current_exe, fs, path::Path};

use codemap::CodeMap;
use codemap_diagnostic::{ColorConfig, Emitter};
use core_plugin_interface::interface::{LibraryLoadingError, SubsystemBuilder};
use error::ParserError;

mod builder;
pub mod error;
pub mod parser;
pub mod typechecker;
mod util;

use core_model_builder::{
    ast::{
        self,
        ast_types::{AstSystem, Untyped},
    },
    error::ModelBuildingError,
};
use regex::Regex;

/// Build a model system from a exo file
pub async fn build_system(
    model_file: impl AsRef<Path>,
    static_builders: Vec<Box<dyn SubsystemBuilder + Send + Sync>>,
) -> Result<Vec<u8>, ParserError> {
    let file_content = fs::read_to_string(model_file.as_ref())?;
    let mut codemap = CodeMap::new();

    codemap.add_file(model_file.as_ref().display().to_string(), file_content);

    build_from_ast_system(
        parser::parse_file(&model_file, &mut codemap),
        codemap,
        static_builders,
    )
    .await
}

// Can we expose this only for testing purposes?
// #[cfg(test)]
pub async fn build_system_from_str(
    model_str: &str,
    file_name: String,
) -> Result<Vec<u8>, ParserError> {
    let mut codemap = CodeMap::new();
    codemap.add_file(file_name.clone(), model_str.to_string());

    build_from_ast_system(
        parser::parse_str(model_str, &mut codemap, &file_name),
        codemap,
        vec![],
    )
    .await
}

pub fn load_subsystem_builders(
    static_builders: Vec<Box<dyn SubsystemBuilder + Send + Sync>>,
) -> Result<Vec<Box<dyn SubsystemBuilder + Send + Sync>>, LibraryLoadingError> {
    let mut dir = current_exe()?;
    dir.pop();

    let pattern = format!(
        "{}(.+)_model_builder_dynamic\\{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    let pattern = Regex::new(&pattern).unwrap();

    let mut subsystem_builders = static_builders;

    for entry in dir.read_dir()?.flatten() {
        if let Some(file_name) = entry.file_name().to_str() {
            let captures = pattern.captures(file_name);
            if let Some(captures) = captures {
                let subsystem_id = captures.get(1).unwrap().as_str();

                // First see if we have already loaded a static builder
                let builder = subsystem_builders
                    .iter()
                    .find(|builder| builder.id() == subsystem_id);

                if builder.is_none() {
                    // Then try to load a dynamic builder
                    subsystem_builders.push(
                        core_plugin_interface::interface::load_subsystem_builder(&entry.path())?,
                    );
                };
            }
        }
    }

    Ok(subsystem_builders)
}

async fn build_from_ast_system(
    ast_system: Result<AstSystem<Untyped>, ParserError>,
    codemap: CodeMap,
    static_builders: Vec<Box<dyn SubsystemBuilder + Send + Sync>>,
) -> Result<Vec<u8>, ParserError> {
    let subsystem_builders = load_subsystem_builders(static_builders)
        .map_err(|e| ParserError::Generic(format!("{e}")))?;

    let ast_system = ast_system.map_err(|err| {
        emit_diagnostics(&err, &codemap);
        err
    })?;

    let typechecked_system =
        typechecker::build(&subsystem_builders, ast_system).map_err(|err| {
            emit_diagnostics(&err, &codemap);
            err
        })?;

    builder::build(&subsystem_builders, typechecked_system)
        .await
        .map_err(|err| {
            let err = err.into();
            emit_diagnostics(&err, &codemap);
            err
        })
}

fn emit_diagnostics(err: &ParserError, codemap: &CodeMap) {
    let mut emitter = Emitter::stderr(ColorConfig::Always, Some(codemap));

    match err {
        ParserError::Diagnosis(diagnostics) => {
            emitter.emit(diagnostics);
        }
        ParserError::ModelBuildingError(ModelBuildingError::Diagnosis(diagnostics)) => {
            emitter.emit(diagnostics);
        }
        ParserError::ModelBuildingError(ModelBuildingError::ExternalResourceParsing(e)) => {
            // This is an error in a JavaScript/TypeScript file, so we
            // have emit it directly to stderr (can't use the emitter, which is tied to exo sources)
            eprintln!("{}", e)
        }
        _ => {}
    }
}
