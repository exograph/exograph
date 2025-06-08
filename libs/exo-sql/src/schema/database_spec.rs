// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{HashMap, HashSet};

use crate::{
    Database, ManyToOne, PhysicalColumn, PhysicalIndex, PhysicalTable, SchemaObjectName, TableId,
    database_error::DatabaseError,
    schema::column_spec::ColumnSpec,
    sql::{connect::database_client::DatabaseClient, relation::RelationColumnPair},
};

use super::{
    column_spec::{ColumnAutoincrement, ColumnDefault, ColumnReferenceSpec, ColumnTypeSpec},
    enum_spec::EnumSpec,
    function_spec::FunctionSpec,
    index_spec::IndexSpec,
    issue::WithIssues,
    op::{RenameTableOp, SchemaOp},
    spec::MigrationScopeMatches,
    table_spec::TableSpec,
    trigger_spec::{TriggerEvent, TriggerOrientation, TriggerSpec, TriggerTiming},
};

const SCHEMAS_QUERY: &str = "SELECT schema_name FROM information_schema.schemata WHERE schema_name != 'information_schema' AND schema_name NOT LIKE 'pg_%' ORDER BY schema_name";

const TABLE_NAMES_QUERY: &str =
    "SELECT table_name FROM information_schema.tables WHERE table_schema = $1 ORDER BY table_name";

const ENUM_NAMES_QUERY: &str = "SELECT t.typname AS enum_name FROM pg_type t JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace 
  WHERE t.typtype = 'e'  AND n.nspname = $1 ORDER BY enum_name";

const MATERIALIZED_VIEWS_QUERY: &str =
    "SELECT matviewname as view_name FROM pg_matviews WHERE schemaname = $1 ORDER BY view_name";

const SEQUENCE_NAMES_QUERY: &str = "SELECT sequence_name FROM information_schema.sequences WHERE sequence_schema = $1 ORDER BY sequence_name";

#[derive(Debug, Clone)]
pub struct DatabaseSpec {
    pub tables: Vec<TableSpec>,
    pub enums: Vec<EnumSpec>,
    pub functions: Vec<FunctionSpec>,
}

impl DatabaseSpec {
    pub fn new(tables: Vec<TableSpec>, enums: Vec<EnumSpec>, functions: Vec<FunctionSpec>) -> Self {
        Self {
            tables,
            enums,
            functions,
        }
    }

    /// Non-public schemas required by this database spec.
    pub fn required_schemas(&self, scope: &MigrationScopeMatches) -> HashSet<String> {
        self.tables
            .iter()
            .filter(|table| scope.matches(&table.name) && table.managed)
            .flat_map(|table| table.name.schema.clone())
            .collect()
    }

    // Explicitly required sequences for this database spec.
    pub fn required_sequences(&self, scope: &MigrationScopeMatches) -> HashSet<SchemaObjectName> {
        self.tables
            .iter()
            .filter(|table| scope.matches(&table.name) && table.managed)
            .flat_map(|table| {
                table
                    .columns
                    .iter()
                    .flat_map(|column| match &column.default_value {
                        Some(ColumnDefault::Autoincrement(ColumnAutoincrement::Sequence {
                            name,
                        })) => Some(name.clone()),
                        _ => None,
                    })
            })
            .collect()
    }

    pub fn needs_public_schema(&self) -> bool {
        self.tables.iter().any(|table| table.name.schema.is_none())
    }

    pub fn required_extensions(&self, scope: &MigrationScopeMatches) -> HashSet<String> {
        self.tables
            .iter()
            .filter(|table| scope.matches(&table.name))
            .fold(HashSet::new(), |acc, table| {
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
            .filter(|table_spec| table_spec.managed)
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
                    is_nullable: column_spec.is_nullable,
                    unique_constraints: column_spec.unique_constraints.to_owned(),
                    default_value: column_spec.default_value.to_owned(),
                    update_sync: false, // There is no good way to know from the database spec if a column should be updated on sync
                    group_name: column_spec.group_name.to_owned(),
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
                        ColumnTypeSpec::ColumnReference(ColumnReferenceSpec {
                            foreign_table_name,
                            foreign_pk_column_name,
                            ..
                        }) => {
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

                            Some(ManyToOne::new(
                                vec![RelationColumnPair {
                                    self_column_id,
                                    foreign_column_id: foreign_pk_column_id,
                                }],
                                foreign_table_alias,
                            ))
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
                    table.managed,
                )
            })
            .collect();

        let enums = database
            .enums()
            .into_iter()
            .map(|(_, enum_)| EnumSpec::new(enum_.name.clone(), enum_.variants.clone()))
            .collect();

        DatabaseSpec::new(tables, enums, all_function_specs)
    }

    /// Creates a new schema specification from an SQL database.
    pub async fn from_live_database(
        client: &DatabaseClient,
        scope: &MigrationScopeMatches,
    ) -> Result<WithIssues<DatabaseSpec>, DatabaseError> {
        let mut issues = Vec::new();
        let mut tables = Vec::new();
        let mut enums = Vec::new();
        let mut sequences = Vec::new();

        let schemas: Vec<String> = client
            .query(SCHEMAS_QUERY, &[])
            .await
            .map_err(DatabaseError::Delegate)?
            .iter()
            .filter_map(|schema_row| {
                let schema: String = schema_row.get("schema_name");
                scope.matches_schema(&schema).then_some(schema)
            })
            .collect::<Vec<_>>();

        for schema_name in &schemas {
            for enum_row in client
                .query(ENUM_NAMES_QUERY, &[&schema_name])
                .await
                .map_err(DatabaseError::Delegate)?
            {
                let enum_name: String = enum_row.get("enum_name");
                let enum_name = SchemaObjectName::new_with_schema_name(enum_name, schema_name);

                let mut enum_ = EnumSpec::from_live_db_enum(client, enum_name).await?;

                issues.append(&mut enum_.issues);
                enums.push(enum_.value);
            }
        }

        let mut column_attributes = HashMap::new();
        for schema_name in &schemas {
            column_attributes.extend(
                ColumnSpec::query_column_attributes(client, schema_name, &enums, &mut issues)
                    .await?,
            );
        }

        for schema_name in schemas {
            for table_row in client
                .query(TABLE_NAMES_QUERY, &[&schema_name])
                .await
                .map_err(DatabaseError::Delegate)?
            {
                let table_name: String = table_row.get("table_name");
                let table_name = SchemaObjectName::new_with_schema_name(table_name, &schema_name);

                let mut table =
                    TableSpec::from_live_db_table(client, table_name, &column_attributes).await?;

                issues.append(&mut table.issues);
                tables.push(table.value);
            }

            for view_row in client
                .query(MATERIALIZED_VIEWS_QUERY, &[&schema_name])
                .await
                .map_err(DatabaseError::Delegate)?
            {
                let view_name: String = view_row.get("view_name");
                let table_name = SchemaObjectName::new_with_schema_name(view_name, &schema_name);

                let mut table =
                    TableSpec::from_live_db_materialized_view(client, table_name, &enums).await?;

                issues.append(&mut table.issues);
                tables.push(table.value);
            }

            for sequence_row in client
                .query(SEQUENCE_NAMES_QUERY, &[&schema_name])
                .await
                .map_err(DatabaseError::Delegate)?
            {
                let sequence_name: String = sequence_row.get("sequence_name");
                sequences.push(SchemaObjectName::new_with_schema_name(
                    sequence_name,
                    &schema_name,
                ));
            }
        }

        let WithIssues {
            value: functions,
            issues: functions_issues,
        } = FunctionSpec::from_live_db(client).await?;
        issues.extend(functions_issues);

        Ok(WithIssues {
            value: DatabaseSpec {
                tables,
                enums,
                functions,
            },
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
                        column.default_value.clone().unwrap().to_sql().unwrap()
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

    pub fn with_table_renamed<'a>(
        &mut self,
        old_name: &'a SchemaObjectName,
        new_name: &'a SchemaObjectName,
    ) -> Vec<SchemaOp<'a>> {
        let mut ops = vec![];

        self.tables.iter_mut().for_each(|table| {
            if &table.name == old_name {
                table.name = new_name.clone();
                ops.push(SchemaOp::RenameTable(RenameTableOp {
                    old_name: old_name.clone(),
                    new_name: new_name.clone(),
                }));

                // Rename sequences used by serial columns
                table.columns.iter().for_each(|column| {
                    if let Some(ColumnDefault::Autoincrement(ColumnAutoincrement::Serial)) =
                        &column.default_value
                    {
                        let old_sequence_name =
                            ColumnAutoincrement::serial_sequence_name(old_name, &column.name);

                        let new_sequence_name =
                            ColumnAutoincrement::serial_sequence_name(new_name, &column.name);

                        ops.push(SchemaOp::RenameSequence {
                            old_name: old_sequence_name,
                            new_name: new_sequence_name,
                        });
                    }
                });
            }

            table.columns.iter_mut().for_each(|column| {
                *column = column.clone().with_table_renamed(old_name, new_name);
            });
        });

        ops
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use crate::schema::column_spec::{ColumnAutoincrement, ColumnDefault};
    use crate::testing::test_support::*;

    use crate::IntBits;

    use super::*;

    #[tokio::test]
    async fn empty_database() {
        test_database_spec("", DatabaseSpec::new(vec![], vec![], vec![])).await;
    }

    #[tokio::test]
    async fn table_with_pk() {
        test_database_spec(
            "CREATE TABLE users (id SERIAL PRIMARY KEY, name VARCHAR(255), email VARCHAR)",
            DatabaseSpec::new(
                vec![TableSpec::new(
                    SchemaObjectName {
                        name: "users".into(),
                        schema: None,
                    },
                    vec![
                        ColumnSpec {
                            name: "id".into(),
                            typ: ColumnTypeSpec::Int { bits: IntBits::_32 },
                            is_pk: true,
                            is_nullable: false,
                            unique_constraints: vec![],
                            default_value: Some(ColumnDefault::Autoincrement(
                                ColumnAutoincrement::Serial,
                            )),
                            group_name: None,
                        },
                        ColumnSpec {
                            name: "name".into(),
                            typ: ColumnTypeSpec::String {
                                max_length: Some(255),
                            },
                            is_pk: false,
                            is_nullable: true,
                            unique_constraints: vec![],
                            default_value: None,
                            group_name: None,
                        },
                        ColumnSpec {
                            name: "email".into(),
                            typ: ColumnTypeSpec::String { max_length: None },
                            is_pk: false,
                            is_nullable: true,
                            unique_constraints: vec![],
                            default_value: None,
                            group_name: None,
                        },
                    ],
                    vec![],
                    vec![],
                    true,
                )],
                vec![],
                vec![],
            ),
        )
        .await;
    }

    #[tokio::test]
    async fn table_without_pk() {
        test_database_spec(
            "CREATE TABLE users (complete BOOLEAN)",
            DatabaseSpec::new(
                vec![TableSpec::new(
                    SchemaObjectName {
                        name: "users".into(),
                        schema: None,
                    },
                    vec![ColumnSpec {
                        name: "complete".into(),
                        typ: ColumnTypeSpec::Boolean,
                        is_pk: false,
                        is_nullable: true,
                        unique_constraints: vec![],
                        default_value: None,
                        group_name: None,
                    }],
                    vec![],
                    vec![],
                    true,
                )],
                vec![],
                vec![],
            ),
        )
        .await;
    }

    #[tokio::test]
    async fn numeric_columns() {
        test_database_spec(
            // One with specified precision and scale, one without
            "CREATE TABLE items (precision_and_scale NUMERIC(10, 2), just_precision NUMERIC(20), no_precision_and_scale NUMERIC)",
            DatabaseSpec::new(
                vec![TableSpec::new(
                    SchemaObjectName {
                        name: "items".into(),
                        schema: None,
                    },
                    vec![
                        ColumnSpec {
                            name: "precision_and_scale".into(),
                            typ: ColumnTypeSpec::Numeric {
                                precision: Some(10),
                                scale: Some(2),
                            },
                            is_pk: false,
                            is_nullable: true,
                            unique_constraints: vec![],
                            default_value: None,
                            group_name: None,
                        },
                        ColumnSpec {
                            name: "just_precision".into(),
                            typ: ColumnTypeSpec::Numeric {
                                precision: Some(20),
                                scale: Some(0), // Default scale for NUMERIC is 0 (https://www.postgresql.org/docs/current/datatype-numeric.html#DATATYPE-NUMERIC-DECIMAL)
                            },
                            is_pk: false,
                            is_nullable: true,
                            unique_constraints: vec![],
                            default_value: None,
                            group_name: None,
                        },
                        ColumnSpec {
                            name: "no_precision_and_scale".into(),
                            typ: ColumnTypeSpec::Numeric {
                                precision: None,
                                scale: None,
                            },
                            is_pk: false,
                            is_nullable: true,
                            unique_constraints: vec![],
                            default_value: None,
                            group_name: None,
                        },
                    ],
                    vec![],
                    vec![],
                    true,
                )],
                vec![],
                vec![],
            ),
        )
        .await;
    }

    async fn test_database_spec(schema: &str, expected_database_spec: DatabaseSpec) {
        with_init_script(schema, |client| async move {
            let WithIssues {
                value: database_spec,
                issues,
            } = DatabaseSpec::from_live_database(&client, &MigrationScopeMatches::all_schemas())
                .await
                .unwrap();

            assert_eq!(issues.len(), 0);

            assert_database_spec_eq(&database_spec, &expected_database_spec);
        })
        .await;
    }

    fn assert_database_spec_eq(actual: &DatabaseSpec, expected: &DatabaseSpec) {
        assert_eq!(actual.tables.len(), expected.tables.len());

        for actual_table in &actual.tables {
            let expected_table = expected.tables.iter().find(|t| t.name == actual_table.name);
            assert!(expected_table.is_some());
            assert_table_spec_eq(actual_table, expected_table.unwrap());
        }

        assert_eq!(actual.functions.len(), expected.functions.len());
        for actual_function in &actual.functions {
            let expected_function = expected
                .functions
                .iter()
                .find(|f| f.name == actual_function.name);
            assert!(expected_function.is_some());
            assert_function_spec_eq(actual_function, expected_function.unwrap());
        }
    }

    fn assert_table_spec_eq(actual: &TableSpec, expected: &TableSpec) {
        assert_eq!(actual.name, expected.name);

        assert_eq!(
            actual.columns.len(),
            expected.columns.len(),
            "Table {:?}: column count mismatch expected {} got {}",
            actual.name,
            expected.columns.len(),
            actual.columns.len()
        );
        for (actual_column, expected_column) in actual.columns.iter().zip(expected.columns.iter()) {
            assert_column_spec_eq(actual_column, expected_column);
        }

        assert_eq!(actual.indices.len(), expected.indices.len());
        for (actual_index, expected_index) in actual.indices.iter().zip(expected.indices.iter()) {
            assert_index_spec_eq(actual_index, expected_index);
        }

        assert_eq!(actual.triggers.len(), expected.triggers.len());
        for (actual_trigger, expected_trigger) in
            actual.triggers.iter().zip(expected.triggers.iter())
        {
            assert_trigger_spec_eq(actual_trigger, expected_trigger);
        }
    }

    fn assert_column_spec_eq(actual: &ColumnSpec, expected: &ColumnSpec) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.typ, expected.typ);
        assert_eq!(actual.is_pk, expected.is_pk);
        assert_eq!(actual.is_nullable, expected.is_nullable);
        assert_eq!(actual.unique_constraints, expected.unique_constraints);
        assert_eq!(actual.default_value, expected.default_value);
    }

    fn assert_index_spec_eq(actual: &IndexSpec, expected: &IndexSpec) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.columns, expected.columns);
        assert_eq!(actual.index_kind, expected.index_kind);
    }

    fn assert_trigger_spec_eq(actual: &TriggerSpec, expected: &TriggerSpec) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.function, expected.function);
        assert_eq!(actual.timing, expected.timing);
        assert_eq!(actual.orientation, expected.orientation);
        assert_eq!(actual.event, expected.event);
    }

    fn assert_function_spec_eq(actual: &FunctionSpec, expected: &FunctionSpec) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.body, expected.body);
        assert_eq!(actual.language, expected.language);
    }
}
