use anyhow::Result;

use exo_sql::schema::database_spec::DatabaseSpec;
use heck::ToUpperCamelCase;

use super::{ImportContext, ModelProcessor};

impl ModelProcessor for DatabaseSpec {
    fn process(
        &self,
        context: &ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()> {
        for schema in &context.schemas {
            let schema = if schema == "public" {
                None
            } else {
                Some(schema.clone())
            };

            let module_name = match &schema {
                Some(schema) => format!("{}Database", schema.to_upper_camel_case()),
                None => "Database".to_string(),
            };

            write!(writer, "@postgres")?;
            if let Some(schema) = &schema {
                write!(writer, "(schema=\"{schema}\")")?;
            }
            writeln!(writer)?;
            writeln!(writer, "module {module_name} {{")?;

            let matching_tables = self
                .tables
                .iter()
                .filter(|table| table.name.schema == schema)
                .collect::<Vec<_>>();

            let table_len = matching_tables.len();

            for (i, table) in matching_tables.iter().enumerate() {
                table.process(context, writer)?;
                if i < table_len - 1 {
                    writeln!(writer)?;
                }
            }

            writeln!(writer, "}}")?;
            writeln!(writer)?;
        }

        Ok(())
    }
}
