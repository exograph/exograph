// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{HashMap, HashSet};

use crate::database_error::DatabaseError;
use crate::schema::constraint::ForeignKeyConstraintColumnPair;
use crate::sql::connect::database_client::DatabaseClient;
use crate::sql::physical_column_type::PhysicalColumnTypeExt;
use crate::{PhysicalTable, SchemaObjectName};

use super::DebugPrintTo;
use super::column_spec::{
    ColumnAttribute, ColumnDefault, ColumnReferenceSpec, ColumnSpec, UuidGenerationMethod,
    physical_column_type_from_string,
};
use super::constraint::{Constraints, sorted_comma_list};
use super::enum_spec::EnumSpec;
use super::index_spec::IndexSpec;
use super::issue::WithIssues;
use super::op::SchemaOp;
use super::statement::SchemaStatement;
use super::trigger_spec::TriggerSpec;

const PHYSICAL_TABLE_COLUMNS_QUERY: &str = "SELECT column_name FROM information_schema.columns WHERE table_name = $1 AND table_schema = $2";

const MATERIALIZED_VIEW_COLUMNS_QUERY: &str = r#"
  SELECT attribute.attname as column_name, pg_catalog.format_type(attribute.atttypid, attribute.atttypmod) as column_type, attribute.attnotnull as not_null 
    FROM pg_attribute attribute JOIN pg_class t on attribute.attrelid = t.oid JOIN pg_namespace schema on t.relnamespace = schema.oid 
  WHERE attribute.attnum > 0 AND NOT attribute.attisdropped AND t.relname = $1 AND schema.nspname = $2"#;

#[derive(Debug, Clone)]
pub struct TableSpec {
    pub name: SchemaObjectName,
    pub columns: Vec<ColumnSpec>,
    pub indices: Vec<IndexSpec>,
    pub triggers: Vec<TriggerSpec>,
    pub managed: bool,
}

impl TableSpec {
    pub fn new(
        name: SchemaObjectName,
        columns: Vec<ColumnSpec>,
        indices: Vec<IndexSpec>,
        triggers: Vec<TriggerSpec>,
        managed: bool,
    ) -> Self {
        Self {
            name,
            columns,
            indices,
            triggers,
            managed,
        }
    }

    pub fn has_single_pk(&self) -> bool {
        self.columns.iter().filter(|c| c.is_pk).count() == 1
    }

    pub fn to_column_less_table(&self) -> PhysicalTable {
        PhysicalTable {
            name: self.name.clone(),
            columns: vec![],
            indices: vec![],
            managed: self.managed,
        }
    }

    pub fn sql_name(&self) -> String {
        self.name.sql_name()
    }

    fn named_unique_constraints(&self) -> HashMap<&String, HashSet<String>> {
        self.columns.iter().fold(HashMap::new(), |mut map, c| {
            {
                for name in c.unique_constraints.iter() {
                    let entry: &mut HashSet<String> = map.entry(name).or_default();
                    (*entry).insert(c.name.clone());
                }
            }
            map
        })
    }

    pub(super) async fn from_live_db_table(
        client: &DatabaseClient,
        table_name: SchemaObjectName,
        column_attributes: &HashMap<SchemaObjectName, HashMap<String, ColumnAttribute>>,
    ) -> Result<WithIssues<TableSpec>, DatabaseError> {
        // Query to get a list of columns in the table

        let mut issues = Vec::new();

        let constraints = Constraints::from_live_db(client, &table_name).await?;

        // Mapping from this table's column name to its reference spec
        let mut column_reference_mapping: HashMap<String, Vec<ColumnReferenceSpec>> =
            HashMap::new();

        for foreign_constraint in constraints.foreign_constraints.into_iter() {
            for column_pair in foreign_constraint.column_pairs.into_iter() {
                let ForeignKeyConstraintColumnPair {
                    self_column,
                    foreign_column,
                } = column_pair;

                let mut column = ColumnSpec::from_live_db(
                    &foreign_constraint.foreign_table,
                    &foreign_column,
                    true, // is_pk = true, since we're referring to a foreign table's PK column (currently, we only support foreign keys to PK columns)
                    None,
                    vec![],
                    column_attributes,
                )
                .await?;

                issues.append(&mut column.issues);

                if let Some(column_spec) = column.value {
                    let column_reference_spec = ColumnReferenceSpec {
                        foreign_table_name: foreign_constraint.foreign_table.clone(),
                        foreign_pk_column_name: foreign_column.clone(),
                        foreign_pk_type: column_spec.typ,
                        group_name: foreign_constraint.constraint_name.clone(),
                    };

                    let existing_ref_specs = column_reference_mapping.get_mut(&self_column);

                    if let Some(existing_ref_specs) = existing_ref_specs {
                        existing_ref_specs.push(column_reference_spec.clone());
                    } else {
                        column_reference_mapping
                            .insert(self_column.clone(), vec![column_reference_spec]);
                    }
                }
            }
        }

        let mut columns = Vec::new();
        for row in client
            .query(
                PHYSICAL_TABLE_COLUMNS_QUERY,
                &[&table_name.name, &table_name.schema_name()],
            )
            .await?
        {
            let column_name: String = row.get("column_name");

            let unique_constraint_names: Vec<_> = constraints
                .uniques
                .iter()
                .flat_map(|unique| {
                    if unique.columns.contains(&column_name) {
                        Some(unique.constraint_name.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let mut column = ColumnSpec::from_live_db(
                &table_name,
                &column_name,
                constraints
                    .primary_key
                    .as_ref()
                    .map(|pk| pk.columns.contains(&column_name))
                    .unwrap_or(false),
                None,
                unique_constraint_names,
                column_attributes,
            )
            .await?;

            issues.append(&mut column.issues);

            if let Some(mut spec) = column.value {
                if let Some(ref_spec) = column_reference_mapping.get(&column_name) {
                    spec.reference_specs = Some(ref_spec.clone());
                }
                columns.push(spec);
            }
        }

        let WithIssues {
            issues: indices_issues,
            value: indices,
        } = IndexSpec::from_live_db(client, &table_name, &columns).await?;
        issues.extend(indices_issues);

        let WithIssues {
            issues: triggers_issues,
            value: triggers,
        } = TriggerSpec::from_live_db(client, &table_name).await?;
        issues.extend(triggers_issues);

        Ok(WithIssues {
            value: TableSpec {
                name: table_name,
                columns,
                indices,
                triggers,
                managed: true,
            },
            issues,
        })
    }

    pub(super) async fn from_live_db_materialized_view(
        client: &DatabaseClient,
        table_name: SchemaObjectName,
        enums: &Vec<EnumSpec>,
    ) -> Result<WithIssues<TableSpec>, DatabaseError> {
        let issues = Vec::new();

        let rows = client
            .query(
                MATERIALIZED_VIEW_COLUMNS_QUERY,
                &[&table_name.name, &table_name.schema_name()],
            )
            .await?;

        let columns = rows
            .iter()
            .map(|row| {
                let name: String = row.get("column_name");
                let typ: String = row.get("column_type");
                let not_null: bool = row.get("not_null");

                let typ = physical_column_type_from_string(&typ, enums).unwrap();

                ColumnSpec {
                    name,
                    typ,
                    reference_specs: None,
                    is_pk: false,
                    is_nullable: !not_null,
                    unique_constraints: vec![],
                    default_value: None,
                }
            })
            .collect();

        Ok(WithIssues {
            value: TableSpec::new(table_name, columns, vec![], vec![], true),
            issues,
        })
    }

    /// Get any extensions this table may depend on.
    pub fn get_required_extensions(&self) -> HashSet<String> {
        let mut required_extensions = HashSet::new();

        for col_spec in self.columns.iter() {
            let typ = &col_spec.typ;
            if typ.is::<crate::sql::physical_column_type::UuidColumnType>() {
                // Only uuid_generate_v4() requires an extension (uuid-ossp)
                // gen_random_uuid() is built into PostgreSQL 13+
                if let Some(ColumnDefault::Uuid(UuidGenerationMethod::V4)) = &col_spec.default_value
                {
                    required_extensions.insert("uuid-ossp".to_string());
                }
            }
            if typ.is::<crate::sql::physical_column_type::VectorColumnType>() {
                required_extensions.insert("vector".to_string());
            }
        }

        required_extensions
    }

    pub fn diff<'a>(&'a self, new: &'a Self) -> Vec<SchemaOp<'a>> {
        // If the exograph model is not managed, we don't need to apply any changes
        if !new.managed {
            return vec![];
        }

        let existing_columns = &self.columns;
        let new_columns = &new.columns;

        let existing_column_map: HashMap<_, _> = existing_columns
            .iter()
            .map(|c| (c.name.clone(), c))
            .collect();
        let new_column_map: HashMap<_, _> =
            new_columns.iter().map(|c| (c.name.clone(), c)).collect();

        let mut changes = vec![];

        for existing_column in self.columns.iter() {
            let new_column = new_column_map.get(&existing_column.name);

            match new_column {
                Some(new_column) => {
                    changes.extend(existing_column.diff(new_column, self, new));
                }
                None => {
                    // column was removed
                    changes.push(SchemaOp::DeleteColumn {
                        table: self,
                        column: existing_column,
                    });
                }
            }
        }

        for new_column in new.columns.iter() {
            let existing_column = existing_column_map.get(&new_column.name);

            if existing_column.is_none() {
                // new column
                changes.push(SchemaOp::CreateColumn {
                    table: new,
                    column: new_column,
                });
            }
        }

        for existing_index in self.indices.iter() {
            let new_index = new
                .indices
                .iter()
                .find(|i| i.name == existing_index.name || i.effectively_eq(existing_index));

            match new_index {
                Some(new_index) => {
                    changes.extend(existing_index.diff(new_index, self, new));
                }
                None => {
                    changes.push(SchemaOp::DeleteIndex {
                        table: self,
                        index: existing_index,
                    });
                }
            }
        }

        for new_index in new.indices.iter() {
            let existing_index = self
                .indices
                .iter()
                .find(|i| i.name == new_index.name || i.effectively_eq(new_index));

            if existing_index.is_none() {
                changes.push(SchemaOp::CreateIndex {
                    table: new,
                    index: new_index,
                });
            }
        }

        let self_unique_constraints = self.named_unique_constraints();
        let new_unique_constraints = new.named_unique_constraints();

        for (self_constraint_name, self_column_names) in self_unique_constraints.iter() {
            let new_constraint_column_names = TableSpec::matching_column_group(
                &new_unique_constraints,
                (self_constraint_name, self_column_names),
            );

            // If the constraint does not exist in the new spec, remove it
            if new_constraint_column_names.is_none() {
                changes.push(SchemaOp::RemoveUniqueConstraint {
                    table: new,
                    constraint: self_constraint_name.to_string(),
                });
            }
        }

        for (new_constraint_name, new_constraint_column_names) in new_unique_constraints.iter() {
            let existing_constraint_column_names = TableSpec::matching_column_group(
                &self_unique_constraints,
                (new_constraint_name, new_constraint_column_names),
            );

            match existing_constraint_column_names {
                Some(existing_constraint_column_names) => {
                    if existing_constraint_column_names != new_constraint_column_names {
                        // constraint modification, so remove the old constraint and add the new one
                        changes.push(SchemaOp::RemoveUniqueConstraint {
                            table: new,
                            constraint: new_constraint_name.to_string(),
                        });
                        changes.push(SchemaOp::CreateUniqueConstraint {
                            table: new,
                            constraint_name: new_constraint_name.to_string(),
                            columns: new_constraint_column_names.clone(),
                        });
                    }
                }
                None => {
                    // new constraint
                    changes.push(SchemaOp::CreateUniqueConstraint {
                        table: new,
                        constraint_name: new_constraint_name.to_string(),
                        columns: new_constraint_column_names.clone(),
                    });
                }
            }
        }

        for trigger in self.triggers.iter() {
            if !new.triggers.iter().any(|t| t.name == trigger.name) {
                // trigger deletion
                changes.push(SchemaOp::DeleteTrigger {
                    trigger,
                    table_name: &self.name,
                });
            }
        }

        for new_trigger in new.triggers.iter() {
            if !self.triggers.iter().any(|t| t.name == new_trigger.name) {
                // new trigger
                changes.push(SchemaOp::CreateTrigger {
                    trigger: new_trigger,
                    table_name: &self.name,
                });
            }
        }

        let self_foreign_key_references = self.foreign_key_references();
        let new_foreign_key_references = new.foreign_key_references();

        // No need to remove the foreign key references since deleting the column will take care of it

        for (column_group_name, column_map) in new_foreign_key_references.into_iter() {
            let existing_column_map_by_group_name = self_foreign_key_references
                .iter()
                .find(|(group_name, _)| group_name == &column_group_name);

            if existing_column_map_by_group_name.is_none() {
                // new foreign key reference
                let new_column_map = column_map
                    .iter()
                    .map(|(column, _)| column.name.clone())
                    .collect::<Vec<_>>();
                let column_map_by_columns =
                    self_foreign_key_references.iter().find(|(_, columns)| {
                        columns
                            .iter()
                            .all(|(column, _)| new_column_map.contains(&column.name))
                    });
                if column_map_by_columns.is_none() {
                    changes.push(SchemaOp::CreateForeignKeyReference {
                        table: new,
                        name: column_group_name,
                        reference_columns: column_map,
                    });
                }
            }
        }

        changes
    }

    /// Converts the table specification to SQL statements.
    pub(super) fn creation_sql(&self) -> SchemaStatement {
        let mut post_statements = Vec::new();

        let column_stmts: String = self
            .columns
            .iter()
            .map(|c| {
                let mut s = c.to_sql(self.has_single_pk());
                post_statements.append(&mut s.post_statements);
                s.statement
            })
            .collect::<Vec<_>>()
            .join(",\n\t");

        let pk_str = if self.has_single_pk() {
            "".to_string()
        } else {
            let pk_columns = self.columns.iter().filter(|c| c.is_pk).collect::<Vec<_>>();

            format!(
                ",\n\tPRIMARY KEY ({})",
                pk_columns
                    .iter()
                    .map(|c| format!("\"{}\"", c.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        let table_name = self.sql_name();

        for (unique_constraint_name, columns) in self.named_unique_constraints().iter() {
            let columns_part = sorted_comma_list(columns, true);

            post_statements.push(format!(
                "ALTER TABLE {table_name} ADD CONSTRAINT \"{unique_constraint_name}\" UNIQUE ({columns_part});"
            ));
        }

        {
            // Add foreign key constraints

            let foreign_key_references = self.foreign_key_references();

            for (column_group_name, column_map) in foreign_key_references.into_iter() {
                let op = SchemaOp::CreateForeignKeyReference {
                    table: self,
                    name: column_group_name,
                    reference_columns: column_map,
                };

                let stmt = op.to_sql();
                post_statements.extend(stmt.post_statements);
            }
        }

        for index in self.indices.iter() {
            post_statements.push(index.creation_sql(&self.name));
        }

        for trigger in self.triggers.iter() {
            post_statements.push(trigger.creation_sql(&self.name));
        }

        SchemaStatement {
            statement: format!("CREATE TABLE {table_name} (\n\t{column_stmts}{pk_str}\n);",),
            pre_statements: vec![],
            post_statements,
        }
    }

    pub(super) fn deletion_sql(&self) -> SchemaStatement {
        let mut pre_statements = vec![];
        for (unique_constraint_name, _) in self.named_unique_constraints().iter() {
            pre_statements.push(format!(
                "ALTER TABLE {} DROP CONSTRAINT \"{}\";",
                self.sql_name(),
                unique_constraint_name
            ));
        }

        SchemaStatement {
            statement: format!("DROP TABLE {} CASCADE;", self.sql_name()),
            pre_statements,
            post_statements: vec![],
        }
    }

    pub fn foreign_key_references(
        &self,
    ) -> Vec<(String, Vec<(&ColumnSpec, &ColumnReferenceSpec)>)> {
        // (column group name/fk name ->  (referring column, foreign column))
        let mut foreign_key_map: HashMap<String, Vec<(&ColumnSpec, &ColumnReferenceSpec)>> =
            HashMap::new();

        for column in self.columns.iter() {
            if let Some(column_references) = &column.reference_specs {
                for column_reference in column_references {
                    foreign_key_map
                        .entry(column_reference.group_name.clone())
                        .or_default()
                        .push((column, column_reference));
                }
            }
        }

        let mut foreign_key_map: Vec<_> = foreign_key_map.into_iter().collect();
        foreign_key_map.sort_by(|(group1, _), (group2, _)| group1.cmp(group2));

        foreign_key_map
            .into_iter()
            .map(|(group_name, mut column_map)| {
                column_map.sort_by(|(column1, _), (column2, _)| column1.name.cmp(&column2.name));
                (group_name, column_map)
            })
            .collect()
    }

    fn matching_column_group<'a>(
        column_groups: &'a HashMap<&String, HashSet<String>>,
        elem: (&String, &HashSet<String>),
    ) -> Option<&'a HashSet<String>> {
        let (group_name, column_names) = elem;

        column_groups.get(group_name).or_else(|| {
            column_groups
                .iter()
                .find_map(|(_, columns)| (columns == column_names).then_some(columns))
        })
    }
}

impl DebugPrintTo for TableSpec {
    fn debug_print_to<W: std::io::Write>(
        &self,
        writer: &mut W,
        indent: usize,
    ) -> std::io::Result<()> {
        let indent_str = " ".repeat(indent);
        writeln!(writer, "{}- Table:", indent_str)?;
        writeln!(
            writer,
            "{}  - Name: {}",
            indent_str,
            self.name.fully_qualified_name()
        )?;

        if !self.columns.is_empty() {
            writeln!(writer, "{}  - Columns:", indent_str)?;
            for column in &self.columns {
                column.debug_print_to(writer, indent + 4)?;
            }
        }

        if !self.indices.is_empty() {
            writeln!(writer, "{}  - Indices:", indent_str)?;
            for index in &self.indices {
                index.debug_print_to(writer, indent + 4)?;
            }
        }

        if !self.triggers.is_empty() {
            writeln!(writer, "{}  - Triggers:", indent_str)?;
            for trigger in &self.triggers {
                trigger.debug_print_to(writer, indent + 4)?;
            }
        }

        let foreign_keys = self.foreign_key_references();
        if !foreign_keys.is_empty() {
            writeln!(writer, "{}  - Foreign Keys:", indent_str)?;
            for (fk_name, columns) in foreign_keys {
                let fk_info = columns
                    .iter()
                    .map(|(col, ref_spec)| {
                        format!(
                            "{} -> {}.{}",
                            col.name,
                            ref_spec.foreign_table_name.fully_qualified_name(),
                            ref_spec.foreign_pk_column_name
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(writer, "{}    - ({}, [{}])", indent_str, fk_name, fk_info)?;
            }
        }

        Ok(())
    }
}
