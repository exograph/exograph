use anyhow::Result;
use exo_sql::{
    SchemaObjectName,
    schema::{database_spec::DatabaseSpec, table_spec::TableSpec},
};
use std::collections::HashSet;

use super::{ImportContext, ModelProcessor, processor::INDENT};

use heck::ToLowerCamelCase;

impl ModelProcessor<DatabaseSpec> for TableSpec {
    fn process(
        &self,
        _parent: &DatabaseSpec,
        context: &ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()> {
        if !context.generate_fragments {
            writeln!(
                writer,
                "{INDENT}@access(query={}, mutation={})",
                context.query_access, context.mutation_access
            )?;

            if !context.has_standard_mapping(&self.name) {
                writeln!(writer, "{INDENT}@table(name=\"{}\")", self.name.name)?;
            }
        }

        let keyword = if context.generate_fragments {
            "fragment"
        } else {
            "type"
        };

        let type_name = {
            let raw_name = context
                .model_name(&self.name)
                .expect("No model name found for table");

            if context.generate_fragments {
                format!("{}Fragment", raw_name)
            } else {
                raw_name.to_string()
            }
        };

        writeln!(writer, "{INDENT}{keyword} {type_name} {{")?;

        // We should only process one column per group (for example, if we have composite primary key)
        let mut processed_groups: HashSet<&str> = HashSet::new();

        for column in &self.columns {
            if let Some(group_name) = &column.group_name {
                if !processed_groups.insert(group_name) {
                    continue;
                }
            }

            column.process(self, context, writer)?;
        }

        write_references(writer, context, &self.name)?;

        writeln!(writer, "{INDENT}}}")?;

        Ok(())
    }
}

fn write_references(
    writer: &mut (dyn std::io::Write + Send),
    context: &ImportContext,
    table_name: &SchemaObjectName,
) -> Result<()> {
    // We should only process one column per group (for example, if we have composite primary key)
    let mut processed_groups: HashSet<&str> = HashSet::new();

    for (table_name, column, _) in context.referenced_columns(table_name) {
        if let Some(group_name) = &column.group_name {
            if !processed_groups.insert(group_name) {
                continue;
            }
        }

        let model_name = context.model_name(&table_name);

        if let Some(model_name) = model_name {
            let is_many = column.unique_constraints.is_empty();
            let field_name = if is_many {
                pluralizer::pluralize(model_name, 2, false)
            } else {
                pluralizer::pluralize(model_name, 1, false)
            }
            .to_lower_camel_case();

            write!(writer, "{INDENT}{INDENT}{field_name}: ")?;

            if is_many {
                write!(writer, "Set<")?;
            }
            write!(writer, "{}", model_name)?;
            if is_many {
                write!(writer, ">")?;
            }

            if column.is_nullable || !is_many {
                write!(writer, "?")?;
            }

            writeln!(writer)?;
        }
    }

    Ok(())
}
