use std::{fs, path::Path};

use codemap::CodeMap;
use codemap_diagnostic::{ColorConfig, Emitter};
use error::ParserError;
use payas_model::model::system::ModelSystem;

mod ast;
mod builder;
pub mod error;
mod parser;
mod typechecker;
mod util;

/// Build a model system from a clay file
pub fn build_system(model_file: impl AsRef<Path>) -> Result<ModelSystem, ParserError> {
    let file_content = fs::read_to_string(model_file.as_ref())?;
    let mut codemap = CodeMap::new();
    codemap.add_file(
        model_file.as_ref().to_str().unwrap().to_string(),
        file_content,
    );

    parser::parse_file(&model_file, &mut codemap)
        .and_then(builder::build)
        .map_err(|err| {
            let mut emitter = Emitter::stderr(ColorConfig::Always, Some(&codemap));
            if let ParserError::Diagnosis(err) = err {
                emitter.emit(&err);
                ParserError::Generic("Failed to parse input file".to_string())
            } else {
                err
            }
        })
}

// Can we expose this only for testing purposes?
// #[cfg(test)]
pub fn build_system_from_str(
    model_str: &str,
    file_name: String,
) -> Result<ModelSystem, ParserError> {
    let mut codemap = CodeMap::new();
    codemap.add_file(file_name.clone(), model_str.to_string());

    parser::parse_str(model_str, &mut codemap, &file_name)
        .and_then(builder::build)
        .map_err(|err| {
            let mut emitter = Emitter::stderr(ColorConfig::Always, Some(&codemap));

            if let ParserError::Diagnosis(err) = err {
                emitter.emit(&err);
                ParserError::Generic("Failed to parse input file".to_string())
            } else {
                err
            }
        })
}
