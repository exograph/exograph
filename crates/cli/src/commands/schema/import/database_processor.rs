use anyhow::Result;

use exo_sql::schema::database_spec::DatabaseSpec;

use super::{ImportContext, ModelProcessor};

impl ModelProcessor for DatabaseSpec {
    fn process(
        &self,
        context: &mut ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()> {
        writeln!(writer, "@postgres")?;
        writeln!(writer, "module Database {{")?;

        for table in &self.tables {
            context.add_table(&table.name);
        }

        for table in &self.tables {
            table.process(context, writer)?;
            writeln!(writer)?;
        }

        writeln!(writer, "}}")?;

        Ok(())
    }
}
