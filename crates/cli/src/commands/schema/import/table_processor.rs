use anyhow::Result;
use exo_sql::schema::{
    column_spec::ColumnSpec, database_spec::DatabaseSpec, table_spec::TableSpec,
};
use std::collections::HashSet;

use super::{
    ImportContext, ModelProcessor, column_processor::write_foreign_key_reference, processor::INDENT,
};

use heck::ToLowerCamelCase;

impl ModelProcessor<DatabaseSpec> for TableSpec {
    fn process(
        &self,
        parent: &DatabaseSpec,
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

        let is_pk = |column_spec: &ColumnSpec| column_spec.is_pk;
        let is_not_pk = |column_spec: &ColumnSpec| !column_spec.is_pk;

        // Categorize columns to determine which should be written as scalars vs consumed by FK references
        let column_categories = categorize_columns(self, context);

        // First write the primary key fields (scalars first, then FKs)
        write_scalar_fields(writer, self, context, &column_categories, &is_pk)?;
        write_foreign_key_reference(writer, context, parent, self, &is_pk)?;

        // Then write the non-primary key fields (scalars first, then FKs)
        write_scalar_fields(writer, self, context, &column_categories, &is_not_pk)?;
        write_foreign_key_reference(writer, context, parent, self, &is_not_pk)?;

        // Finally write back-references (such as Set<User>, User?, etc.) for which this table is the target
        write_back_references(writer, context, &column_categories)?;

        writeln!(writer, "{INDENT}}}")?;

        Ok(())
    }
}

struct ColumnCategories<'a> {
    /// Columns that should be written as scalar fields
    scalar_columns: HashSet<&'a str>,
    /// Columns that are consumed by FK references (won't be written as scalars)
    #[allow(dead_code)]
    fk_consumed_columns: HashSet<&'a str>,
    /// Back-reference fields with complete information (deduplicated)
    back_reference_fields: Vec<(String, String, bool, bool)>, // (field_name, model_name, is_many, is_nullable)
}

fn categorize_columns<'a>(
    table_spec: &'a TableSpec,
    context: &ImportContext,
) -> ColumnCategories<'a> {
    let pk_columns: HashSet<&str> = table_spec
        .columns
        .iter()
        .filter(|c| c.is_pk)
        .map(|c| c.name.as_str())
        .collect();

    let fk_references = table_spec.foreign_key_references();

    // Columns that are consumed by FK references (won't be written as scalars)
    let mut fk_consumed_columns = HashSet::new();

    // Process each FK to determine which columns it consumes
    for (_, references) in &fk_references {
        if references.len() == 1 && pk_columns.contains(references[0].0.name.as_str()) {
            // Single-column FK on a PK column - this column is consumed by the FK
            fk_consumed_columns.insert(references[0].0.name.as_str());
        } else {
            // Composite FK or non-PK FK - non-PK columns are consumed
            for (col, _) in references {
                if !col.is_pk {
                    fk_consumed_columns.insert(col.name.as_str());
                }
            }
        }
    }

    // All columns that aren't consumed by FKs should be written as scalars
    let scalar_columns: HashSet<&str> = table_spec
        .columns
        .iter()
        .filter(|c| !fk_consumed_columns.contains(c.name.as_str()))
        .map(|c| c.name.as_str())
        .collect();

    // Compute back-reference fields with complete information (deduplicated)
    let mut seen_back_ref_names = HashSet::new();
    let mut back_reference_fields = Vec::new();

    for (ref_table_name, column, _) in context.referenced_columns(&table_spec.name) {
        if let Some(model_name) = context.model_name(&ref_table_name) {
            let is_many = column.unique_constraints.is_empty();
            let field_name = if is_many {
                pluralizer::pluralize(model_name, 2, false)
            } else {
                pluralizer::pluralize(model_name, 1, false)
            }
            .to_lower_camel_case();

            if seen_back_ref_names.insert(field_name.clone()) {
                let is_nullable = column.is_nullable || !is_many;
                back_reference_fields.push((
                    field_name,
                    model_name.to_string(),
                    is_many,
                    is_nullable,
                ));
            }
        }
    }

    ColumnCategories {
        scalar_columns,
        fk_consumed_columns,
        back_reference_fields,
    }
}

fn write_scalar_fields(
    writer: &mut (dyn std::io::Write + Send),
    table_spec: &TableSpec,
    context: &ImportContext,
    column_categories: &ColumnCategories,
    filter: &dyn Fn(&ColumnSpec) -> bool,
) -> Result<()> {
    for column in &table_spec.columns {
        // Write this column as a scalar field if:
        // 1. It's in the scalar_columns set (not consumed by FK)
        // 2. It matches the filter (PK or non-PK)
        if column_categories
            .scalar_columns
            .contains(column.name.as_str())
            && filter(column)
        {
            column.process(table_spec, context, writer)?;
        }
    }

    Ok(())
}

fn write_back_references(
    writer: &mut (dyn std::io::Write + Send),
    _context: &ImportContext,
    column_categories: &ColumnCategories,
) -> Result<()> {
    // Write back-references using pre-computed deduplicated information
    for (field_name, model_name, is_many, is_nullable) in &column_categories.back_reference_fields {
        write!(writer, "{INDENT}{INDENT}{field_name}: ")?;

        if *is_many {
            write!(writer, "Set<")?;
        }
        write!(writer, "{}", model_name)?;
        if *is_many {
            write!(writer, ">")?;
        }

        if *is_nullable {
            write!(writer, "?")?;
        }

        writeln!(writer)?;
    }

    Ok(())
}
