use anyhow::Result;
use exo_sql::schema::{
    column_spec::ColumnSpec, database_spec::DatabaseSpec, table_spec::TableSpec,
};
use std::collections::{HashMap, HashSet};
use std::io::Write;

use super::column_processor::FieldImportKind;
use super::{
    ImportContext,
    column_processor::FieldImport,
    traits::{ImportWriter, ModelImporter},
};

use heck::{ToLowerCamelCase, ToSnakeCase, ToUpperCamelCase};

const INDENT: &str = "  ";

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
pub struct BackReferenceField {
    pub field_name: String,
    pub model_name: String,
    pub is_many: bool,
    pub is_nullable: bool,
    pub relation_name: Option<String>,
}

#[derive(Debug)]
pub struct ColumnCategories<'a> {
    pub scalar_columns: HashSet<&'a str>,
    #[allow(dead_code)]
    pub fk_consumed_columns: HashSet<&'a str>,
    pub back_reference_fields: Vec<BackReferenceField>,
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

        // Categorize columns to determine which should be written as scalars vs consumed by FK references
        let column_categories = self.categorize_columns(context);

        let mut fields = Vec::new();

        // Add scalar fields and foreign key references (these columns exist in this table)
        self.add_scalar_fields(&mut fields, context, &column_categories)?;
        self.add_foreign_key_references(&mut fields, context, parent)?;

        // Add back-references (such as Set<User>, User?, etc.) for which this table is the target
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

impl ImportWriter for TableImport {
    fn write_to(self, writer: &mut (dyn Write + Send)) -> Result<()> {
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
        let mut fields = self.fields;

        // Sort fields (within each category: ordered by type, nullable, and name):
        // - PK fields
        //   - Scalars
        //   - References
        // - Non-PK fields
        //   - Scalars
        //   - References
        //   - Back-references
        //     - One
        //     - Many
        fields.sort_by(|a, b| {
            let a_kind = &a.field_kind;
            let b_kind = &b.field_kind;

            let scalar_pk_cmp = || {
                matches!(b_kind, FieldImportKind::Scalar { is_pk: true })
                    .cmp(&matches!(a_kind, FieldImportKind::Scalar { is_pk: true }))
            };
            let reference_pk_cmp = || {
                matches!(b_kind, FieldImportKind::Reference { is_pk: true }).cmp(&matches!(
                    a_kind,
                    FieldImportKind::Reference { is_pk: true }
                ))
            };
            let scalar_non_pk_cmp = || {
                matches!(b_kind, FieldImportKind::Scalar { is_pk: false })
                    .cmp(&matches!(a_kind, FieldImportKind::Scalar { is_pk: false }))
            };
            let reference_non_pk_cmp = || {
                matches!(b_kind, FieldImportKind::Reference { is_pk: false }).cmp(&matches!(
                    a_kind,
                    FieldImportKind::Reference { is_pk: false }
                ))
            };
            let back_reference_one_cmp = || {
                matches!(b_kind, FieldImportKind::BackReference { is_many: false }).cmp(&matches!(
                    a_kind,
                    FieldImportKind::BackReference { is_many: false }
                ))
            };
            let back_reference_many_cmp = || {
                matches!(b_kind, FieldImportKind::BackReference { is_many: true }).cmp(&matches!(
                    a_kind,
                    FieldImportKind::BackReference { is_many: true }
                ))
            };
            let type_cmp = || a.data_type.cmp(&b.data_type);
            let nullable_cmp = || a.is_nullable.cmp(&b.is_nullable);
            let name_cmp = || a.name.cmp(&b.name);

            scalar_pk_cmp()
                .then_with(reference_pk_cmp)
                .then_with(scalar_non_pk_cmp)
                .then_with(reference_non_pk_cmp)
                .then_with(back_reference_one_cmp)
                .then_with(back_reference_many_cmp)
                .then_with(type_cmp)
                .then_with(nullable_cmp)
                .then_with(name_cmp)
        });

        for field in fields {
            field.write_to(writer)?;
        }

        writeln!(writer, "{INDENT}}}")?;

        Ok(())
    }
}

/// Trait for import-specific functionality on TableSpec
trait TableSpecImportNaming {
    /// Categorize columns to determine which should be written as scalars vs consumed by FK references
    fn categorize_columns<'a>(&'a self, context: &ImportContext) -> ColumnCategories<'a>;

    /// Add scalar fields to the field list
    fn add_scalar_fields(
        &self,
        fields: &mut Vec<FieldImport>,
        context: &ImportContext,
        column_categories: &ColumnCategories,
    ) -> Result<()>;

    /// Add foreign key reference fields to the field list
    fn add_foreign_key_references(
        &self,
        fields: &mut Vec<FieldImport>,
        context: &ImportContext,
        database_spec: &DatabaseSpec,
    ) -> Result<()>;

    /// Get all index names that contain the specified column
    fn get_indices_for_column(&self, column_name: &str) -> Vec<&str>;

    /// Generate index annotation for a column, handling both single and multi-column indices
    fn generate_index_annotation(&self, column_name: &str) -> Option<String>;
}

impl TableSpecImportNaming for TableSpec {
    fn categorize_columns<'a>(&'a self, context: &ImportContext) -> ColumnCategories<'a> {
        let pk_columns: HashSet<&str> = self
            .columns
            .iter()
            .filter(|c| c.is_pk)
            .map(|c| c.name.as_str())
            .collect();

        let fk_references = self.foreign_key_references();

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
        let scalar_columns: HashSet<&str> = self
            .columns
            .iter()
            .filter(|c| !fk_consumed_columns.contains(c.name.as_str()))
            .map(|c| c.name.as_str())
            .collect();

        // Compute back-reference fields with complete information (deduplicated)
        let mut seen_back_ref_names = HashSet::new();
        let mut back_reference_fields = Vec::new();

        // For multi-relation back-references, we need to check if the source table
        // has multiple foreign keys pointing to this table
        let mut source_table_fk_counts: HashMap<String, Vec<String>> = HashMap::new();

        // First pass: count foreign keys by analyzing each source table
        for (ref_table_name, _) in context.referenced_columns(&self.name) {
            // Find the source table and count its FKs to this table
            if let Some(source_table) = context
                .database_spec
                .tables
                .iter()
                .find(|t| t.name == ref_table_name)
            {
                let fks_to_this_table: Vec<_> = source_table
                    .foreign_key_references()
                    .into_iter()
                    .filter(|(_, refs)| {
                        refs.iter()
                            .any(|(_, ref_spec)| ref_spec.foreign_table_name == self.name)
                    })
                    .collect();

                source_table_fk_counts.insert(
                    ref_table_name.fully_qualified_name(),
                    fks_to_this_table
                        .into_iter()
                        .map(|(fk_name, _)| fk_name)
                        .collect(),
                );
            }
        }

        for (ref_table_name, column) in context.referenced_columns(&self.name) {
            if let Some(model_name) = context.model_name(&ref_table_name) {
                let is_many = column.unique_constraints.is_empty();
                let has_multiple_relations = source_table_fk_counts
                    .get(&ref_table_name.fully_qualified_name())
                    .is_some_and(|source_fks| source_fks.len() > 1);

                let (field_name, relation_name) = if has_multiple_relations {
                    // Multiple relations from source table to this table
                    let relation_name = compute_relation_name(&column.name);
                    let source_model = context.model_name(&ref_table_name).unwrap();
                    let target_model = context.model_name(&self.name).unwrap();
                    let field_name = compute_back_reference_field_name(
                        &relation_name,
                        source_model,
                        target_model,
                        is_many,
                    );
                    (field_name, Some(relation_name.to_lower_camel_case()))
                } else {
                    // Single relation - use standard naming
                    let field_name = if is_many {
                        pluralizer::pluralize(model_name, 2, false)
                    } else {
                        pluralizer::pluralize(model_name, 1, false)
                    }
                    .to_lower_camel_case();
                    (field_name, None)
                };

                if seen_back_ref_names.insert(field_name.clone()) {
                    let is_nullable = !is_many && column.is_nullable;
                    back_reference_fields.push(BackReferenceField {
                        field_name,
                        model_name: model_name.to_string(),
                        is_many,
                        is_nullable,
                        relation_name,
                    });
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
        &self,
        fields: &mut Vec<FieldImport>,
        context: &ImportContext,
        column_categories: &ColumnCategories,
    ) -> Result<()> {
        for column in &self.columns {
            // Add this column as a scalar field if:
            // 1. It's in the scalar_columns set (not consumed by FK)
            // 2. It matches the filter (PK or non-PK)
            if column_categories
                .scalar_columns
                .contains(column.name.as_str())
            {
                let mut field_import = column.to_import(self, context)?;

                // Add @index annotation if this column has an index
                if let Some(index_annotation) = self.generate_index_annotation(&column.name) {
                    field_import.annotations.push(index_annotation);
                }

                fields.push(field_import);
            }
        }

        Ok(())
    }

    fn add_foreign_key_references(
        &self,
        fields: &mut Vec<FieldImport>,
        context: &ImportContext,
        database_spec: &DatabaseSpec,
    ) -> Result<()> {
        // Group foreign keys by target table
        let mut fks_by_target: HashMap<String, Vec<_>> = HashMap::new();

        for (_, references) in self.foreign_key_references() {
            let (_, first_reference) = match &references[..] {
                [] => {
                    continue;
                }
                [reference, ..] => reference,
            };

            let foreign_table_name = &first_reference.foreign_table_name;
            fks_by_target
                .entry(foreign_table_name.fully_qualified_name())
                .or_insert_with(Vec::new)
                .push(references);
        }

        // Process each group of FKs
        for (_foreign_table_key, fk_group) in fks_by_target {
            let has_multiple = fk_group.len() > 1;

            for references in fk_group.into_iter() {
                let (first_column, first_reference) = &references[0];

                // Assert that all references point to the same table
                let all_references_point_to_same_table = references.iter().all(|(_, reference)| {
                    reference.foreign_table_name == first_reference.foreign_table_name
                });
                if !all_references_point_to_same_table {
                    return Err(anyhow::anyhow!(
                        "All foreign key references from column '{}' in table '{}' must point to the same foreign table (this is likely a programming error)",
                        first_column.name,
                        self.name.fully_qualified_name()
                    ));
                }

                let foreign_table_name = &first_reference.foreign_table_name;

                // Generate field name based on whether we have multiple relations
                let field_name = if has_multiple {
                    // Generate field name for foreign key references in multi-relation scenarios
                    let relation_name = compute_relation_name(&first_column.name);
                    relation_name.to_lower_camel_case()
                } else {
                    context.get_composite_foreign_key_field_name(foreign_table_name)
                };

                let data_type = context
                    .model_name(foreign_table_name)
                    .ok_or(anyhow::anyhow!(
                        "No model name found for foreign table '{}' referenced from table '{}'",
                        foreign_table_name.fully_qualified_name(),
                        self.name.fully_qualified_name()
                    ))?
                    .to_string();

                let is_pk = references.iter().all(|(col, _)| col.is_pk);
                let is_unique = references
                    .iter()
                    .all(|(col, _)| !col.unique_constraints.is_empty());
                let is_nullable = references.iter().any(|(col, _)| col.is_nullable);

                let mut annotations = Vec::new();

                // Check if we need an annotation
                if references.len() == 1 {
                    // Single column FK - check if annotation is needed
                    let col_name = &references[0].0.name;
                    let expected_col_name = format!("{}_id", field_name.to_snake_case());
                    let pk_col_name = &first_reference.foreign_pk_column_name;

                    // Check if this is an irregular table (PK column name ends with "_id")
                    let is_irregular_table = pk_col_name.ends_with("_id");

                    // Add annotation if:
                    // 1. Expected column name doesn't match actual column name, OR
                    // 2. This is an irregular table (where PK columns end with "_id")
                    if expected_col_name != *col_name || is_irregular_table {
                        annotations.push(format!("@column(\"{}\")", col_name));
                    }
                } else {
                    // Multi-column FK - use the existing mapping logic
                    let mapping_annotation = reference_mapping_annotation(
                        &field_name,
                        &references,
                        database_spec,
                        context,
                    );
                    if let Some(mapping) = mapping_annotation {
                        annotations.push(mapping);
                    }
                }

                // Add @index annotation if any of the FK columns have an index
                // For FK fields, we need to check the foreign key column name (not the field name)
                if let Some(fk_column) = references.first() {
                    if let Some(index_annotation) =
                        self.generate_index_annotation(&fk_column.0.name)
                    {
                        annotations.push(index_annotation);
                    }
                }

                fields.push(FieldImport {
                    name: field_name,
                    data_type,
                    field_kind: FieldImportKind::Reference { is_pk },
                    is_unique,
                    is_nullable,
                    annotations,
                    default_value: None, // Foreign key references don't have default values
                });
            }
        }

        Ok(())
    }

    fn get_indices_for_column(&self, column_name: &str) -> Vec<&str> {
        self.indices
            .iter()
            .filter(|index| index.columns.contains(column_name))
            .map(|index| index.name.as_str())
            .collect()
    }

    fn generate_index_annotation(&self, column_name: &str) -> Option<String> {
        let mut index_names = self.get_indices_for_column(column_name);
        if index_names.is_empty() {
            return None;
        }

        // If there's only one index and it's a single-column index, use simple @index annotation
        if index_names.len() == 1 {
            let index_name = index_names[0];
            let index = self
                .indices
                .iter()
                .find(|idx| idx.name == index_name)
                .unwrap();
            if index.columns.len() == 1 {
                return Some("@index".to_string());
            }
        }

        // Sort indices: single-column indices first, then multi-column indices
        index_names.sort_by_key(|&name| {
            let index = self.indices.iter().find(|idx| idx.name == name).unwrap();
            index.columns.len()
        });

        // For multiple indices or multi-column indices, generate full annotation with index names
        let index_list = index_names
            .iter()
            .map(|name| format!("\"{}\"", name))
            .collect::<Vec<_>>()
            .join(", ");

        Some(format!("@index({})", index_list))
    }
}

/// Extract relation name from column name by removing common ID suffixes
///
/// # Examples
/// - `derive_relation_name("account_id")` -> "account"
/// - `derive_relation_name("counterparty_account_id")` -> "counterparty_account"  
/// - `derive_relation_name("userId")` -> "user"
/// - `derive_relation_name("status")` -> "status"
fn compute_relation_name(column_name: &str) -> String {
    column_name
        .strip_suffix("_id")
        .or_else(|| column_name.strip_suffix("Id"))
        .unwrap_or(column_name)
        .to_string()
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

/// Generate field name for back-reference in multi-relation scenarios
///
/// # Examples
/// - `generate_back_reference_field_name("account", "Transaction", "Account", true)` -> "transactions"
/// - `generate_back_reference_field_name("counterparty_account", "Transaction", "Account", true)` -> "counterpartyTransactions"
/// - `generate_back_reference_field_name("user_account", "Post", "Account", false)` -> "userAccount" (one-to-one)
fn compute_back_reference_field_name(
    relation_name: &str,
    source_model: &str,
    target_model: &str,
    is_many: bool,
) -> String {
    if is_many {
        // Generate field name for Set back-references using naming heuristics
        let plural_source = pluralizer::pluralize(source_model, 2, false);
        let target_model_lower = target_model.to_lowercase();

        if relation_name.to_lowercase() == target_model_lower {
            // e.g. "account" relation to Account model -> just "transactions"
            plural_source.to_lower_camel_case()
        } else if relation_name.ends_with(&format!("_{}", target_model_lower)) {
            // e.g. "counterparty_account" -> extract "counterparty" + "Transactions"
            let prefix = &relation_name[..relation_name.len() - target_model_lower.len() - 1];
            format!(
                "{}{}",
                prefix.to_lower_camel_case(),
                plural_source.to_upper_camel_case()
            )
        } else {
            // Fallback: use full relation name + plural source
            format!(
                "{}{}",
                relation_name.to_lower_camel_case(),
                plural_source.to_upper_camel_case()
            )
        }
    } else {
        relation_name.to_lower_camel_case()
    }
}

fn add_back_references(
    fields: &mut Vec<FieldImport>,
    column_categories: &ColumnCategories,
) -> Result<()> {
    // Add back-references using pre-computed deduplicated information
    for back_ref in &column_categories.back_reference_fields {
        let data_type = if back_ref.is_many {
            format!("Set<{}>", back_ref.model_name)
        } else {
            back_ref.model_name.clone()
        };

        let mut annotations = Vec::new();
        if let Some(rel_name) = &back_ref.relation_name {
            annotations.push(format!("@relation(\"{}\")", rel_name));
        }

        let field_kind = FieldImportKind::BackReference {
            is_many: back_ref.is_many,
        };

        fields.push(FieldImport {
            name: back_ref.field_name.clone(),
            data_type,
            field_kind,
            is_unique: false,
            is_nullable: back_ref.is_nullable,
            annotations,
            default_value: None,
        });
    }

    Ok(())
}
