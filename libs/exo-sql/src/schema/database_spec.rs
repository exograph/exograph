// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use crate::{
    database_error::DatabaseError, schema::column_spec::ColumnSpec,
    sql::connect::database_client::DatabaseClient, Database, ManyToOne, PhysicalColumn,
    PhysicalIndex, PhysicalTable, PhysicalTableName, TableId,
};

use super::{
    column_spec::ColumnTypeSpec,
    function_spec::FunctionSpec,
    index_spec::IndexSpec,
    issue::WithIssues,
    table_spec::TableSpec,
    trigger_spec::{TriggerEvent, TriggerOrientation, TriggerSpec, TriggerTiming},
};

#[derive(Debug)]
pub struct DatabaseSpec {
    pub tables: Vec<TableSpec>,
    pub functions: Vec<FunctionSpec>,
}

impl DatabaseSpec {
    pub fn new(tables: Vec<TableSpec>, functions: Vec<FunctionSpec>) -> Self {
        Self { tables, functions }
    }

    /// Non-public schemas required by this database spec.
    pub fn required_schemas(&self) -> HashSet<String> {
        self.tables
            .iter()
            .flat_map(|table| table.name.schema.clone())
            .collect()
    }

    pub fn required_extensions(&self) -> HashSet<String> {
        self.tables.iter().fold(HashSet::new(), |acc, table| {
            acc.union(&table.get_required_extensions())
                .cloned()
                .collect()
        })
    }

    pub fn to_database(self) -> Database {
        let mut database = Database::default();

        // Step 1: Create tables (without columns)
        let tables: Vec<(TableId, Vec<ColumnSpec>, Vec<IndexSpec>)> = self
            .tables
            .into_iter()
            .map(|table| {
                let table_id = database.insert_table(table.to_column_less_table());
                (table_id, table.columns, table.indices)
            })
            .collect();

        // Step 2: Add columns to tables
        for (table_id, column_specs, index_specs) in tables.iter() {
            let columns = column_specs
                .iter()
                .map(|column_spec| PhysicalColumn {
                    table_id: *table_id,
                    name: column_spec.name.to_owned(),
                    typ: column_spec.typ.to_database_type(),
                    is_pk: column_spec.is_pk,
                    is_auto_increment: column_spec.is_auto_increment,
                    is_nullable: column_spec.is_nullable,
                    unique_constraints: column_spec.unique_constraints.to_owned(),
                    default_value: column_spec.default_value.to_owned(),
                    update_sync: false, // There is no good way to know from the database spec if a column should be updated on sync
                })
                .collect();

            let table = database.get_table_mut(*table_id);

            table.columns = columns;
            table.indices = index_specs
                .iter()
                .map(|index_spec| PhysicalIndex {
                    name: index_spec.name.to_owned(),
                    columns: index_spec.columns.to_owned(),
                    index_kind: index_spec.index_kind.to_owned(),
                })
                .collect();
        }

        // Step 3: Add relations to the database
        let relations: Vec<ManyToOne> = tables
            .iter()
            .flat_map(|(table_id, column_specs, _)| {
                let table = database.get_table(*table_id);

                let column_ids = database.get_column_ids(*table_id);

                column_ids.into_iter().flat_map(|self_column_id| {
                    let column = &table.columns[self_column_id.column_index];
                    let column_spec = column_specs
                        .iter()
                        .find(|column_spec| column_spec.name == column.name)
                        .unwrap();

                    match &column_spec.typ {
                        ColumnTypeSpec::ColumnReference {
                            foreign_table_name,
                            foreign_pk_column_name,
                            ..
                        } => {
                            let foreign_table_id =
                                database.get_table_id(foreign_table_name).unwrap();
                            let foreign_pk_column_id = database
                                .get_column_id(foreign_table_id, foreign_pk_column_name)
                                .unwrap();
                            // Roughly match the behavior in type_builder.rs, where we set up the
                            // alias to the pluralized field name, which in typical setup matches
                            // the table name.

                            // TODO: Make unit tests compare statements semantically, not lexically
                            // so setting up aliases consistently is same as not setting them up in
                            // case aliases are unnecessary.
                            let foreign_table_alias = Some(if column.name.ends_with("_id") {
                                let base_name = &column.name[..column.name.len() - 3];
                                let plural_suffix =
                                    if base_name.ends_with('s') { "es" } else { "s" };
                                format!("{base_name}{plural_suffix}")
                            } else {
                                column.name.clone()
                            });

                            Some(ManyToOne {
                                self_column_id,
                                foreign_pk_column_id,
                                foreign_table_alias,
                            })
                        }
                        _ => None,
                    }
                })
            })
            .collect();

        database.relations = relations;

        database
    }

    pub fn from_database(database: &Database) -> DatabaseSpec {
        let mut all_function_specs = vec![];

        let tables = database
            .tables()
            .into_iter()
            .map(|(_, table)| {
                let (trigger_specs, function_specs) = match Self::update_trigger(table) {
                    Some((trigger, function)) => (vec![trigger], vec![function]),
                    None => (vec![], vec![]),
                };

                all_function_specs.extend(function_specs);

                TableSpec::new(
                    table.name.clone(),
                    table
                        .columns
                        .clone()
                        .into_iter()
                        .map(|c| ColumnSpec::from_physical(c, database))
                        .collect(),
                    table
                        .indices
                        .clone()
                        .into_iter()
                        .map(|index| IndexSpec {
                            name: index.name,
                            columns: index.columns.into_iter().collect(),
                            index_kind: index.index_kind,
                        })
                        .collect(),
                    trigger_specs,
                )
            })
            .collect();

        DatabaseSpec::new(tables, all_function_specs)
    }

    /// Creates a new schema specification from an SQL database.
    pub async fn from_live_database(
        client: &DatabaseClient,
    ) -> Result<WithIssues<DatabaseSpec>, DatabaseError> {
        const SCHEMAS_QUERY: &str =
            "SELECT DISTINCT table_schema FROM information_schema.tables WHERE table_schema != 'information_schema' AND table_schema NOT LIKE 'pg_%' AND table_type <> 'SYSTEM VIEW'";

        // Query to get a list of all the tables in the database
        const TABLE_NAMES_QUERY: &str =
            "SELECT table_name FROM information_schema.tables WHERE table_schema = $1";

        let mut issues = Vec::new();
        let mut tables = Vec::new();

        for schema_row in client
            .query(SCHEMAS_QUERY, &[])
            .await
            .map_err(DatabaseError::Delegate)?
        {
            let raw_schema_name: String = schema_row.get("table_schema");
            let schema_name = if raw_schema_name == "public" {
                None
            } else {
                Some(raw_schema_name.clone())
            };

            for table_row in client
                .query(TABLE_NAMES_QUERY, &[&raw_schema_name])
                .await
                .map_err(DatabaseError::Delegate)?
            {
                let table_name = PhysicalTableName {
                    name: table_row.get("table_name"),
                    schema: schema_name.clone(),
                };

                let mut table = TableSpec::from_live_db(client, table_name).await?;
                issues.append(&mut table.issues);
                tables.push(table.value);
            }
        }

        let WithIssues {
            value: functions,
            issues: functions_issues,
        } = FunctionSpec::from_live_db(client).await?;
        issues.extend(functions_issues);

        Ok(WithIssues {
            value: DatabaseSpec { tables, functions },
            issues,
        })
    }

    fn update_trigger(table: &PhysicalTable) -> Option<(TriggerSpec, FunctionSpec)> {
        let update_sync_columns = table
            .columns
            .iter()
            .filter(|column| column.update_sync)
            .collect::<Vec<_>>();

        if !update_sync_columns.is_empty() {
            let table_name = table.name.fully_qualified_name_with_sep("_");

            let update_statements = update_sync_columns
                .iter()
                .map(|column| {
                    format!(
                        "NEW.{} = {}",
                        column.name,
                        column.default_value.clone().unwrap()
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");

            let function_name = format!("exograph_update_{table_name}");
            let function_body = format!("BEGIN {update_statements}; RETURN NEW; END;");

            let trigger_name = format!("exograph_on_update_{}", table_name);

            Some((
                TriggerSpec {
                    name: trigger_name,
                    function: function_name.clone(),
                    timing: TriggerTiming::Before,
                    orientation: TriggerOrientation::Row,
                    event: TriggerEvent::Update,
                    table: table.name.clone(),
                },
                FunctionSpec {
                    name: function_name,
                    body: function_body,
                    language: "plpgsql".into(),
                },
            ))
        } else {
            None
        }
    }
}
