use anyhow::Result;
use exo_sql::{schema::table_spec::TableSpec, PhysicalTableName};

use super::{ImportContext, ModelProcessor};

const INDENT: &str = "  ";
impl ModelProcessor for TableSpec {
    fn process(
        &self,
        context: &mut ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()> {
        writeln!(writer, "{INDENT}@access({})", context.access)?;

        match (&self.name.schema, context.has_standard_mapping(&self.name)) {
            (Some(schema), true) => writeln!(writer, "{INDENT}@table(schema=\"{}\")", schema)?,
            (Some(schema), false) => writeln!(
                writer,
                "{INDENT}@table(name=\"{}\", schema=\"{}\")",
                self.name.name, schema
            )?,
            (None, false) => writeln!(writer, "{INDENT}@table(\"{}\")", self.name.name)?,
            (None, true) => {}
        }

        writeln!(writer, "{INDENT}type {} {{", context.model_name(&self.name))?;

        for column in &self.columns {
            column.process(context, writer)?;
        }

        write_references(writer, context, &self.name)?;

        writeln!(writer, "{INDENT}}}")?;

        Ok(())
    }
}

fn write_references(
    writer: &mut (dyn std::io::Write + Send),
    context: &ImportContext,
    table_name: &PhysicalTableName,
) -> Result<()> {
    for (table_name, column, _) in context.referenced_columns(table_name) {
        let is_many = column.unique_constraints.is_empty();
        let field_name = if is_many {
            table_name.name.to_string()
        } else {
            pluralizer::pluralize(&table_name.name, 1, false)
        };

        write!(writer, "{INDENT}{INDENT}{field_name}: ")?;

        if is_many {
            write!(writer, "Set<")?;
        }
        write!(writer, "{}", context.model_name(&table_name))?;
        if is_many {
            write!(writer, ">")?;
        }

        if column.is_nullable || !is_many {
            write!(writer, "?")?;
        }

        writeln!(writer)?;
    }

    Ok(())
}
