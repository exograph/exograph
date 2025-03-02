use anyhow::Result;
use exo_sql::schema::table_spec::TableSpec;

use super::{ImportContext, ModelProcessor};

impl ModelProcessor for TableSpec {
    fn process(
        &self,
        context: &mut ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()> {
        writeln!(writer, "\t@access({})", context.access)?;

        if !context.has_standard_mapping(&self.name) {
            match &self.name.schema {
                Some(schema) => writeln!(
                    writer,
                    "\t@table(name=\"{}\", schema=\"{}\")",
                    self.name.name, schema
                )?,
                None => writeln!(writer, "\t@table(\"{}\")", self.name.name)?,
            };
        }

        writeln!(writer, "\ttype {} {{", context.model_name(&self.name))?;

        for column in &self.columns {
            column.process(context, writer)?;
        }

        writeln!(writer, "\t}}")?;

        Ok(())
    }
}
