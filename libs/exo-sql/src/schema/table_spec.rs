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
use crate::sql::connect::database_client::DatabaseClient;
use crate::{PhysicalTable, PhysicalTableName};

use super::column_spec::{ColumnSpec, ColumnTypeSpec};
use super::constraint::{sorted_comma_list, Constraints};
use super::index_spec::IndexSpec;
use super::issue::WithIssues;
use super::op::SchemaOp;
use super::statement::SchemaStatement;
use super::trigger_spec::TriggerSpec;

#[derive(Debug)]
pub struct TableSpec {
    pub name: PhysicalTableName,
    pub columns: Vec<ColumnSpec>,
    pub indices: Vec<IndexSpec>,
    pub triggers: Vec<TriggerSpec>,
    pub tracked: bool,
}

impl TableSpec {
    pub fn new(
        name: PhysicalTableName,
        columns: Vec<ColumnSpec>,
        indices: Vec<IndexSpec>,
        triggers: Vec<TriggerSpec>,
        tracked: bool,
    ) -> Self {
        Self {
            name,
            columns,
            indices,
            triggers,
            tracked,
        }
    }

    pub fn to_column_less_table(&self) -> PhysicalTable {
        PhysicalTable {
            name: self.name.clone(),
            columns: vec![],
            indices: vec![],
            tracked: self.tracked,
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

    /// Creates a new table specification from an SQL table.
    pub(super) async fn from_live_db(
        client: &DatabaseClient,
        table_name: PhysicalTableName,
    ) -> Result<WithIssues<TableSpec>, DatabaseError> {
        // Query to get a list of columns in the table
        let columns_query = format!(
            "SELECT column_name FROM information_schema.columns WHERE table_name = '{}' AND table_schema = '{}'",
            table_name.name, table_name.schema.as_ref().unwrap_or(&"public".to_string())
        );

        let mut issues = Vec::new();

        let constraints = Constraints::from_live_db(client, &table_name).await?;

        let mut column_type_mapping = HashMap::new();

        for foreign_constraint in constraints.foreign_constraints.iter() {
            // Assumption that there is only one column in the foreign key (for now a correct assumption since we don't support composite keys)
            let self_column_name = foreign_constraint.self_columns.iter().next().unwrap();
            let foreign_pk_column_name = foreign_constraint.foreign_columns.iter().next().unwrap();

            let mut column = ColumnSpec::from_live_db(
                client,
                &table_name,
                foreign_pk_column_name,
                true,
                None,
                vec![],
            )
            .await?;
            issues.append(&mut column.issues);

            if let Some(spec) = column.value {
                column_type_mapping.insert(
                    self_column_name.clone(),
                    ColumnTypeSpec::ColumnReference {
                        foreign_table_name: foreign_constraint.foreign_table.clone(),
                        foreign_pk_column_name: foreign_pk_column_name.clone(),
                        foreign_pk_type: Box::new(spec.typ),
                    },
                );
            }
        }

        let mut columns = Vec::new();
        for row in client.query(columns_query.as_str(), &[]).await? {
            let name: String = row.get("column_name");

            let unique_constraint_names: Vec<_> = constraints
                .uniques
                .iter()
                .flat_map(|unique| {
                    if unique.columns.contains(&name) {
                        Some(unique.constraint_name.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let mut column = ColumnSpec::from_live_db(
                client,
                &table_name,
                &name,
                constraints
                    .primary_key
                    .as_ref()
                    .map(|pk| pk.columns.contains(&name))
                    .unwrap_or(false),
                column_type_mapping.get(&name).cloned(),
                unique_constraint_names,
            )
            .await?;
            issues.append(&mut column.issues);

            if let Some(spec) = column.value {
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
                tracked: true,
            },
            issues,
        })
    }

    /// Get any extensions this table may depend on.
    pub fn get_required_extensions(&self) -> HashSet<String> {
        let mut required_extensions = HashSet::new();

        for col_spec in self.columns.iter() {
            if let ColumnTypeSpec::Uuid = col_spec.typ {
                required_extensions.insert("pgcrypto".to_string());
            }
            if let ColumnTypeSpec::Vector { .. } = col_spec.typ {
                required_extensions.insert("vector".to_string());
            }
        }

        required_extensions
    }

    pub fn diff<'a>(&'a self, new: &'a Self) -> Vec<SchemaOp<'a>> {
        // If the exograph model is not tracked, we don't need to apply any changes
        if !new.tracked {
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
            let new_index = new.indices.iter().find(|i| i.name == existing_index.name);

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
            let existing_index = self.indices.iter().find(|i| i.name == new_index.name);

            if existing_index.is_none() {
                changes.push(SchemaOp::CreateIndex {
                    table: new,
                    index: new_index,
                });
            }
        }

        for (constraint_name, _column_names) in self.named_unique_constraints().iter() {
            if !new.named_unique_constraints().contains_key(constraint_name) {
                // constraint deletion
                changes.push(SchemaOp::RemoveUniqueConstraint {
                    table: new,
                    constraint: constraint_name.to_string(),
                });
            }
        }

        for (new_constraint_name, new_constraint_column_names) in
            new.named_unique_constraints().iter()
        {
            let existing_constraints = self.named_unique_constraints();
            let existing_constraint_column_names = existing_constraints.get(new_constraint_name);

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
                changes.push(SchemaOp::DeleteTrigger { trigger });
            }
        }

        for new_trigger in new.triggers.iter() {
            if !self.triggers.iter().any(|t| t.name == new_trigger.name) {
                // new trigger
                changes.push(SchemaOp::CreateTrigger {
                    trigger: new_trigger,
                });
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
                let mut s = c.to_sql(self);
                post_statements.append(&mut s.post_statements);
                s.statement
            })
            .collect::<Vec<_>>()
            .join(",\n\t");

        let table_name = self.sql_name();

        for (unique_constraint_name, columns) in self.named_unique_constraints().iter() {
            let columns_part = sorted_comma_list(columns, true);

            post_statements.push(format!(
                "ALTER TABLE {table_name} ADD CONSTRAINT \"{unique_constraint_name}\" UNIQUE ({columns_part});"
            ));
        }

        for index in self.indices.iter() {
            post_statements.push(index.creation_sql(&self.name));
        }

        for trigger in self.triggers.iter() {
            post_statements.push(trigger.creation_sql());
        }

        SchemaStatement {
            statement: format!("CREATE TABLE {table_name} (\n\t{column_stmts}\n);"),
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
}
