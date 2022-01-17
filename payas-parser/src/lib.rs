use std::{fs, path::Path};

use codemap::CodeMap;
use codemap_diagnostic::{ColorConfig, Emitter};
use error::ParserError;
use payas_model::model::system::ModelSystem;

mod ast;
mod builder;
pub(crate) mod error;
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
    let mut emitter = Emitter::stderr(ColorConfig::Always, Some(&codemap));

    parser::parse_file(&model_file)
        .and_then(builder::build)
        .map_err(|err| {
            if let ParserError::Diagnosis(err) = err {
                emitter.emit(&err);
                ParserError::Generic("Failed to parse input file".to_string())
            } else {
                err
            }
        })
}
