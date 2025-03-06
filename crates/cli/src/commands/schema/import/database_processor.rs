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

            write!(writer, "@postgres")?;
            if let Some(schema) = &schema {
                if !context.generate_fragments {
                    write!(writer, "(schema=\"{schema}\")")?;
                }
            }
            writeln!(writer)?;

            let module_suffix = if context.generate_fragments {
                "Fragments"
            } else {
                "Database"
            };

            let module_name = match &schema {
                Some(schema) => format!("{}{}", schema.to_upper_camel_case(), module_suffix),
                None => module_suffix.to_string(),
            };
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
