use anyhow::Result;
use exo_sql::{
    SchemaObjectName,
    schema::{column_spec::ColumnSpec, database_spec::DatabaseSpec, table_spec::TableSpec},
};
use std::collections::HashSet;

use super::{
    ImportContext, ModelProcessor, column_processor::write_foreign_key_reference, processor::INDENT,
};

use heck::ToLowerCamelCase;

impl ModelProcessor<DatabaseSpec, ()> for TableSpec {
    fn process(
        &self,
        parent: &DatabaseSpec,
        context: &ImportContext,
        _parent_context: &mut (),
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

        // Fields that have already been added to the model
        let mut processed_fields: HashSet<String> = HashSet::new();

        let is_pk = |column_spec: &ColumnSpec| column_spec.is_pk;
        let is_not_pk = |column_spec: &ColumnSpec| !column_spec.is_pk;

        // First write the primary key fields
        write_scalar_fields(writer, self, context, &mut processed_fields, &is_pk)?;
        write_foreign_key_reference(writer, context, parent, self, &mut processed_fields, &is_pk)?;

        // Then write the non-primary key fields
        write_scalar_fields(writer, self, context, &mut processed_fields, &is_not_pk)?;
        write_foreign_key_reference(
            writer,
            context,
            parent,
            self,
            &mut processed_fields,
            &is_not_pk,
        )?;

        // Finally write the references
        write_references(writer, context, &mut processed_fields, &self.name)?;

        writeln!(writer, "{INDENT}}}")?;

        Ok(())
    }
}

fn write_scalar_fields(
    writer: &mut (dyn std::io::Write + Send),
    table_spec: &TableSpec,
    context: &ImportContext,
    processed_fields: &mut HashSet<String>,
    filter: &dyn Fn(&ColumnSpec) -> bool,
) -> Result<()> {
    for column in &table_spec.columns {
        if filter(column) {
            column.process(table_spec, context, processed_fields, writer)?;
        }
    }

    Ok(())
}

fn write_references(
    writer: &mut (dyn std::io::Write + Send),
    context: &ImportContext,
    processed_fields: &mut HashSet<String>,
    table_name: &SchemaObjectName,
) -> Result<()> {
    for (table_name, column, _) in context.referenced_columns(table_name) {
        let model_name = context.model_name(&table_name);

        if let Some(model_name) = model_name {
            let is_many = column.unique_constraints.is_empty();
            let field_name = if is_many {
                pluralizer::pluralize(model_name, 2, false)
            } else {
                pluralizer::pluralize(model_name, 1, false)
            }
            .to_lower_camel_case();

            if !processed_fields.insert(field_name.clone()) {
                // Skip fields that have already been added to the model
                continue;
            }

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
