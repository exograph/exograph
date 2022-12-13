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

/// Build a model system from a clay file
pub fn build_system(model_file: impl AsRef<Path>) -> Result<Vec<u8>, ParserError> {
    let file_content = fs::read_to_string(model_file.as_ref())?;
    let mut codemap = CodeMap::new();
    codemap.add_file(
        model_file.as_ref().to_str().unwrap().to_string(),
        file_content,
    );

    build_from_ast_system(parser::parse_file(&model_file, &mut codemap), codemap)
}

// Can we expose this only for testing purposes?
// #[cfg(test)]
pub fn build_system_from_str(model_str: &str, file_name: String) -> Result<Vec<u8>, ParserError> {
    let mut codemap = CodeMap::new();
    codemap.add_file(file_name.clone(), model_str.to_string());

    build_from_ast_system(
        parser::parse_str(model_str, &mut codemap, &file_name),
        codemap,
    )
}

pub fn load_subsystem_builders() -> Result<Vec<Box<dyn SubsystemBuilder>>, LibraryLoadingError> {
    let mut dir = current_exe()?;
    dir.pop();

    let pattern = format!(
        "{}(.+)_model_builder\\{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    let pattern = Regex::new(&pattern).unwrap();

    let mut subsystem_builders = vec![];

    for entry in dir.read_dir()?.flatten() {
        if let Some(file_name) = entry.file_name().to_str() {
            if pattern.is_match(file_name) {
                subsystem_builders.push(core_plugin_interface::interface::load_subsystem_builder(
                    &entry.path(),
                )?);
            }
        }
    }

    Ok(subsystem_builders)
}

fn build_from_ast_system(
    ast_system: Result<AstSystem<Untyped>, ParserError>,
    codemap: CodeMap,
) -> Result<Vec<u8>, ParserError> {
    let subsystem_builders =
        load_subsystem_builders().map_err(|e| ParserError::Generic(format!("{}", e)))?;

    ast_system
        .and_then(|ast_system| typechecker::build(&subsystem_builders, ast_system))
        .and_then(|typechecked_system| {
            builder::build(&subsystem_builders, typechecked_system).map_err(|e| e.into())
        })
        .map_err(|err| {
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
        _ => {}
    }
}
