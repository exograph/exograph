use std::path::Path;

use payas_model::model::system::ModelSystem;

mod ast;
mod builder;
mod parser;
mod typechecker;
mod util;
use anyhow::Result;

/// Build a model system from a clay file
pub fn build_system(model_file: impl AsRef<Path>) -> Result<ModelSystem> {
    let (ast_system, codemap) = parser::parse_file(&model_file)?;
    builder::build(ast_system, codemap)
}
