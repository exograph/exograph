use anyhow::Result;
use exo_sql::schema::{database_spec::DatabaseSpec, enum_spec::EnumSpec};

use super::{ImportContext, ModelProcessor, processor::INDENT};

use heck::ToUpperCamelCase;

impl ModelProcessor<DatabaseSpec> for EnumSpec {
    fn process(
        &self,
        _parent: &DatabaseSpec,
        _context: &ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()> {
        writeln!(
            writer,
            "{INDENT}enum {} {{",
            self.name.name.to_upper_camel_case()
        )?;
        for value in &self.variants {
            writeln!(writer, "{INDENT}{INDENT}{value}")?;
        }
        writeln!(writer, "{INDENT}}}")?;
        Ok(())
    }
}
