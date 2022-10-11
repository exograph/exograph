use std::{fs, path::Path};

use codemap::CodeMap;
use codemap_diagnostic::{ColorConfig, Emitter};
use error::ParserError;

mod builder;
pub mod error;
mod parser;
mod typechecker;
mod util;

use payas_core_model_builder::{
    ast::{
        self,
        ast_types::{AstSystem, Untyped},
    },
    error::ModelBuildingError,
};

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

fn build_from_ast_system(
    ast_system: Result<AstSystem<Untyped>, ParserError>,
    codemap: CodeMap,
) -> Result<Vec<u8>, ParserError> {
    ast_system
        .and_then(typechecker::build)
        .and_then(|types| builder::build(types).map_err(|e| e.into()))
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
