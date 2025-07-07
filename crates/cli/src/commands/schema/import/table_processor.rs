use anyhow::Result;
use exo_sql::schema::{
    column_spec::ColumnSpec, database_spec::DatabaseSpec, table_spec::TableSpec,
};
use std::collections::HashSet;
use std::io::Write;

use super::{
    ImportContext,
    column_processor::FieldImport,
    traits::{ImportWriter, ModelImporter},
};

const INDENT: &str = "  ";

use heck::ToLowerCamelCase;

#[derive(Debug)]
pub struct TableImport {
    pub name: String,
    pub is_fragment: bool,
    pub access_annotation: Option<AccessAnnotation>,
    pub table_annotation: Option<String>,
    pub fields: Vec<FieldImport>,
}

#[derive(Debug)]
pub struct AccessAnnotation {
    pub query: bool,
    pub mutation: bool,
}

#[derive(Debug)]
pub struct ColumnCategories<'a> {
    pub scalar_columns: HashSet<&'a str>,
    #[allow(dead_code)]
    pub fk_consumed_columns: HashSet<&'a str>,
    pub back_reference_fields: Vec<(String, String, bool, bool)>, // (field_name, model_name, is_many, is_nullable)
}

impl ModelImporter<DatabaseSpec, TableImport> for TableSpec {
    fn to_import(&self, parent: &DatabaseSpec, context: &ImportContext) -> Result<TableImport> {
        let access_annotation = if !context.generate_fragments {
            Some(AccessAnnotation {
                query: context.query_access,
                mutation: context.mutation_access,
            })
        } else {
            None
        };

        let table_annotation =
            if !context.generate_fragments && !context.has_standard_mapping(&self.name) {
                Some(format!("@table(name=\"{}\")", self.name.name))
            } else {
                None
            };

        let raw_name = context
            .model_name(&self.name)
            .expect("No model name found for table");

        let name = if context.generate_fragments {
            format!("{}Fragment", raw_name)
        } else {
            raw_name.to_string()
        };

        let is_pk = |column_spec: &ColumnSpec| column_spec.is_pk;
        let is_not_pk = |column_spec: &ColumnSpec| !column_spec.is_pk;

        // Categorize columns to determine which should be written as scalars vs consumed by FK references
        let column_categories = categorize_columns(self, context);

        let mut fields = Vec::new();

        // First add the primary key fields (scalars first, then FKs)
        add_scalar_fields(&mut fields, self, context, &column_categories, &is_pk)?;
        add_foreign_key_references(&mut fields, context, parent, self, &is_pk)?;

        // Then add the non-primary key fields (scalars first, then FKs)
        add_scalar_fields(&mut fields, self, context, &column_categories, &is_not_pk)?;
        add_foreign_key_references(&mut fields, context, parent, self, &is_not_pk)?;

        // Finally add back-references (such as Set<User>, User?, etc.) for which this table is the target
        add_back_references(&mut fields, &column_categories)?;

        Ok(TableImport {
            name,
            is_fragment: context.generate_fragments,
            access_annotation,
            table_annotation,
            fields,
        })
    }
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

fn add_scalar_fields(
    fields: &mut Vec<FieldImport>,
    table_spec: &TableSpec,
    context: &ImportContext,
    column_categories: &ColumnCategories,
    filter: &dyn Fn(&ColumnSpec) -> bool,
) -> Result<()> {
    for column in &table_spec.columns {
        // Add this column as a scalar field if:
        // 1. It's in the scalar_columns set (not consumed by FK)
        // 2. It matches the filter (PK or non-PK)
        if column_categories
            .scalar_columns
            .contains(column.name.as_str())
            && filter(column)
        {
            fields.push(column.to_import(table_spec, context)?);
        }
    }

    Ok(())
}

fn add_foreign_key_references(
    fields: &mut Vec<FieldImport>,
    context: &ImportContext,
    database_spec: &DatabaseSpec,
    table_spec: &TableSpec,
    filter: &dyn Fn(&ColumnSpec) -> bool,
) -> Result<()> {
    for (_, references) in table_spec.foreign_key_references() {
        let (first_column, first_reference) = match &references[..] {
            [] => {
                continue;
            }
            [reference, ..] => reference,
        };

        // Only process this foreign key if the first column matches the filter
        // This determines when we write the FK (during PK pass or non-PK pass)
        if !filter(first_column) {
            continue;
        }

        // Assert that all references point to the same table
        let all_references_point_to_same_table = references.iter().all(|(_, reference)| {
            reference.foreign_table_name == first_reference.foreign_table_name
        });
        if !all_references_point_to_same_table {
            return Err(anyhow::anyhow!(
                "All references from {} in {} must point to the same foreign table (this is like a programming error)",
                references[0].0.name,
                table_spec.name.fully_qualified_name()
            ));
        }

        let foreign_table_name = &first_reference.foreign_table_name;
        let field_name = context.get_composite_foreign_key_field_name(foreign_table_name);

        let data_type = context
            .model_name(foreign_table_name)
            .ok_or(anyhow::anyhow!(
                "No model name found for foreign table name: {:?}",
                foreign_table_name
            ))?
            .to_string();

        let mapping_annotation =
            reference_mapping_annotation(&field_name, &references, database_spec, context);

        let is_pk = references.iter().all(|(col, _)| col.is_pk);
        let is_unique = references
            .iter()
            .all(|(col, _)| !col.unique_constraints.is_empty());
        let is_nullable = references.iter().any(|(col, _)| col.is_nullable);

        let mut annotations = Vec::new();
        if let Some(mapping) = mapping_annotation {
            annotations.push(mapping);
        }

        fields.push(FieldImport {
            name: field_name,
            data_type,
            is_pk,
            is_unique,
            is_nullable,
            annotations,
            default_value: None, // Foreign key references don't have default values
        });
    }

    Ok(())
}

fn add_back_references(
    fields: &mut Vec<FieldImport>,
    column_categories: &ColumnCategories,
) -> Result<()> {
    // Add back-references using pre-computed deduplicated information
    for (field_name, model_name, is_many, is_nullable) in &column_categories.back_reference_fields {
        let data_type = if *is_many {
            format!("Set<{}>", model_name)
        } else {
            model_name.clone()
        };

        fields.push(FieldImport {
            name: field_name.clone(),
            data_type,
            is_pk: false,
            is_unique: false,
            is_nullable: *is_nullable,
            annotations: Vec::new(),
            default_value: None,
        });
    }

    Ok(())
}

fn reference_mapping_annotation(
    field_name: &str,
    references: &Vec<(
        &ColumnSpec,
        &exo_sql::schema::column_spec::ColumnReferenceSpec,
    )>,
    database_spec: &DatabaseSpec,
    context: &ImportContext,
) -> Option<String> {
    let mut mapping_pairs = Vec::new();

    for (col, reference) in references {
        let foreign_table = database_spec
            .tables
            .iter()
            .find(|t| t.name == reference.foreign_table_name)
            .unwrap();
        let foreign_column_spec = foreign_table
            .columns
            .iter()
            .find(|c| c.name == reference.foreign_pk_column_name)
            .unwrap();

        let (foreign_field_name, needs_mapping) = match &foreign_column_spec.reference_specs {
            Some(foreign_reference_specs) => {
                let name = context.get_composite_foreign_key_field_name(
                    &foreign_reference_specs[0].foreign_table_name,
                );
                let needs_mapping = name != col.name;
                (name, needs_mapping)
            }
            None => {
                let name = context.standard_field_name(&reference.foreign_pk_column_name);
                let default_field_name =
                    format!("{field_name}_{}", reference.foreign_pk_column_name);

                let needs_mapping = default_field_name != col.name;
                (name, needs_mapping)
            }
        };

        if needs_mapping {
            mapping_pairs.push((foreign_field_name, col.name.clone()));
        }
    }

    match &mapping_pairs[..] {
        [] => None,
        [mapping_pair] => {
            let mapping_annotation = format!("@column(\"{}\")", mapping_pair.1);
            Some(mapping_annotation)
        }
        _ => {
            let mapping_annotation = format!(
                "@column(mapping={{{}}})",
                mapping_pairs
                    .iter()
                    .map(|(k, v)| format!("{}: \"{}\"", k, v))
                    .collect::<Vec<String>>()
                    .join(", ")
            );
            Some(mapping_annotation)
        }
    }
}

impl ImportWriter for TableImport {
    fn write_to(&self, writer: &mut (dyn Write + Send)) -> Result<()> {
        // Write access annotation
        if let Some(access) = &self.access_annotation {
            writeln!(
                writer,
                "{INDENT}@access(query={}, mutation={})",
                access.query, access.mutation
            )?;
        }

        // Write table annotation
        if let Some(table_annotation) = &self.table_annotation {
            writeln!(writer, "{INDENT}{}", table_annotation)?;
        }

        // Write type/fragment declaration
        let keyword = if self.is_fragment { "fragment" } else { "type" };
        writeln!(writer, "{INDENT}{keyword} {} {{", self.name)?;

        // Write fields
        for field in &self.fields {
            field.write_to(writer)?;
        }

        writeln!(writer, "{INDENT}}}")?;

        Ok(())
    }
}
