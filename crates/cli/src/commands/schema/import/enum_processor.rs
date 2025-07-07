use anyhow::Result;
use exo_sql::schema::{database_spec::DatabaseSpec, enum_spec::EnumSpec};
use std::io::Write;

use super::{
    ImportContext,
    traits::{ImportWriter, ModelImporter},
};

use heck::ToUpperCamelCase;

const INDENT: &str = "  ";

#[derive(Debug)]
pub struct EnumImport {
    pub name: String,
    pub variants: Vec<String>,
}

impl ModelImporter<DatabaseSpec, EnumImport> for EnumSpec {
    fn to_import(&self, _parent: &DatabaseSpec, _context: &ImportContext) -> Result<EnumImport> {
        Ok(EnumImport {
            name: self.name.name.to_upper_camel_case(),
            variants: self.variants.clone(),
        })
    }
}

impl ImportWriter for EnumImport {
    fn write_to(&self, writer: &mut (dyn Write + Send)) -> Result<()> {
        writeln!(writer, "{INDENT}enum {} {{", self.name)?;

        for variant in &self.variants {
            writeln!(writer, "{INDENT}{INDENT}{}", variant)?;
        }

        writeln!(writer, "{INDENT}}}")?;

        Ok(())
    }
}
