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

        let table_len = self.tables.len();

        for (i, table) in self.tables.iter().enumerate() {
            table.process(context, writer)?;
            if i < table_len - 1 {
                writeln!(writer)?;
            }
        }

        writeln!(writer, "}}")?;

        Ok(())
    }
}
