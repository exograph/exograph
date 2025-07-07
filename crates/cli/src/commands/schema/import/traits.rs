//! Core traits for the import module.
//!
//! This module defines traits used throughout the import system:
//! - `ModelImporter`: Converts database models into import structures
//! - `ImportWriter`: Serializes import structures to output

use anyhow::Result;
use std::io::Write;

use super::ImportContext;

/// A trait for converting a model into import structures.
///
/// `P` is the parent type, `O` is the output type.
pub(super) trait ModelImporter<P, O> {
    fn to_import(&self, parent: &P, context: &ImportContext) -> Result<O>;
}

/// A trait for writing import structures to output
pub(super) trait ImportWriter {
    fn write_to(&self, writer: &mut (dyn Write + Send)) -> Result<()>;
}
