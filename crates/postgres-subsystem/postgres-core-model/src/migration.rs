// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt::Display;

use exo_sql::{
    database_error::DatabaseError,
    schema::{
        database_spec::DatabaseSpec,
        issue::WithIssues,
        op::SchemaOp,
        spec::{diff, MigrationScope, MigrationScopeMatches},
    },
    Database, DatabaseClientManager,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Migration {
    pub statements: Vec<MigrationStatement>,
}

#[derive(Debug, Serialize)]
pub struct MigrationStatement {
    pub statement: String,
    pub is_destructive: bool,
}

pub enum VerificationErrors {
    PostgresError(DatabaseError),
    ModelNotCompatible(Vec<String>),
}

impl From<DatabaseError> for VerificationErrors {
    fn from(e: DatabaseError) -> Self {
        VerificationErrors::PostgresError(e)
    }
}

impl Display for VerificationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationErrors::PostgresError(e) => write!(f, "Postgres error: {e}"),
            VerificationErrors::ModelNotCompatible(e) => {
                for error in e.iter() {
                    writeln!(f, "- {error}")?
                }

                Ok(())
            }
        }
    }
}

impl Migration {
    pub fn from_schemas(
        old_schema_spec: &DatabaseSpec,
        new_schema_spec: &DatabaseSpec,
        scope: &MigrationScope,
    ) -> Self {
        let mut pre_statements = vec![];
        let mut statements = vec![];
        let mut post_statements = vec![];

        let diffs = diff(old_schema_spec, new_schema_spec, scope);

        for diff in diffs.iter() {
            let is_destructive = match diff {
                SchemaOp::DeleteSchema { .. }
                | SchemaOp::DeleteTable { .. }
                | SchemaOp::DeleteColumn { .. }
                | SchemaOp::RemoveExtension { .. } => true,

                // Explicitly matching the other cases here to ensure that we have thought about each case
                SchemaOp::CreateSchema { .. }
                | SchemaOp::CreateTable { .. }
                | SchemaOp::CreateColumn { .. }
                | SchemaOp::CreateIndex { .. }
                | SchemaOp::DeleteIndex { .. } // Creating and deleting index is not considered destructive (they affect performance but not data loss)
                | SchemaOp::CreateExtension { .. }
                | SchemaOp::CreateUniqueConstraint { .. }
                | SchemaOp::RemoveUniqueConstraint { .. }
                | SchemaOp::SetColumnDefaultValue { .. }
                | SchemaOp::UnsetColumnDefaultValue { .. }
                | SchemaOp::SetNotNull { .. }
                | SchemaOp::UnsetNotNull { .. }
                | SchemaOp::CreateFunction { .. }
                | SchemaOp::DeleteFunction { .. }
                | SchemaOp::CreateOrReplaceFunction { .. }
                | SchemaOp::CreateTrigger { .. }
                | SchemaOp::DeleteTrigger { .. } => false,
            };

            let statement = diff.to_sql();

            for constraint in statement.pre_statements.into_iter() {
                pre_statements.push(MigrationStatement::new(constraint, is_destructive));
            }

            statements.push(MigrationStatement::new(statement.statement, is_destructive));

            for constraint in statement.post_statements.into_iter() {
                post_statements.push(MigrationStatement::new(constraint, is_destructive));
            }
        }

        pre_statements.extend(statements);
        pre_statements.extend(post_statements);

        Migration {
            statements: pre_statements,
        }
    }

    pub async fn from_db_and_model(
        client: &DatabaseClientManager,
        database: &Database,
        scope: &MigrationScope,
    ) -> Result<Self, DatabaseError> {
        let database_spec = DatabaseSpec::from_database(database);

        let scope_matches = match scope {
            MigrationScope::Specified(scope) => scope,
            MigrationScope::FromNewSpec => {
                &MigrationScopeMatches::from_specs_schemas(&[&database_spec])
            }
        };

        let old_schema = extract_db_schema(client, scope_matches).await?;

        for issue in &old_schema.issues {
            eprintln!("{issue}");
        }

        Ok(Migration::from_schemas(
            &old_schema.value,
            &database_spec,
            scope,
        ))
    }

    pub fn has_destructive_changes(&self) -> bool {
        self.statements
            .iter()
            .any(|statement| statement.is_destructive)
    }

    pub async fn verify(
        client: &DatabaseClientManager,
        database: &Database,
        scope: &MigrationScope,
    ) -> Result<(), VerificationErrors> {
        let new_schema = DatabaseSpec::from_database(database);

        let scope_matches = match scope {
            MigrationScope::Specified(scope) => scope,
            MigrationScope::FromNewSpec => {
                &MigrationScopeMatches::from_specs_schemas(&[&new_schema])
            }
        };

        let old_schema = extract_db_schema(client, scope_matches).await?;

        for issue in &old_schema.issues {
            eprintln!("{issue}");
        }

        let diff = diff(&old_schema.value, &new_schema, scope);

        let errors: Vec<_> = diff.iter().flat_map(|op| op.error_string()).collect();

        if !errors.is_empty() {
            Err(VerificationErrors::ModelNotCompatible(errors))
        } else {
            Ok(())
        }
    }

    pub async fn apply(
        &self,
        database: &DatabaseClientManager,
        allow_destructive_changes: bool,
    ) -> Result<(), anyhow::Error> {
        let mut client = database.get_client().await?;
        let transaction = client.transaction().await?;
        for MigrationStatement {
            statement,
            is_destructive,
        } in self.statements.iter()
        {
            if !is_destructive || allow_destructive_changes {
                transaction.execute(statement, &[]).await?;
            } else {
                return Err(anyhow::anyhow!(
                    "Destructive change detected: {}",
                    statement
                ));
            }
        }
        Ok(transaction.commit().await?)
    }

    pub fn write(
        &self,
        writer: &mut dyn std::io::Write,
        allow_destructive_changes: bool,
    ) -> std::io::Result<()> {
        for MigrationStatement {
            statement,
            is_destructive,
        } in self.statements.iter()
        {
            if *is_destructive && !allow_destructive_changes {
                write!(writer, "-- ")?;
            }
            writeln!(writer, "{statement}\n")?;
        }
        Ok(())
    }
}

impl MigrationStatement {
    pub fn new(statement: String, is_destructive: bool) -> Self {
        Self {
            statement,
            is_destructive,
        }
    }
}

async fn extract_db_schema(
    database: &DatabaseClientManager,
    scope: &MigrationScopeMatches,
) -> Result<WithIssues<DatabaseSpec>, DatabaseError> {
    let client = database.get_client().await?;

    DatabaseSpec::from_live_database(&client, scope).await
}

pub async fn wipe_database(database: &DatabaseClientManager) -> Result<(), DatabaseError> {
    let client = database.get_client().await?;

    // wiping is equivalent to migrating to an empty database and deals with non-public schemas correctly
    let current_database_spec =
        &DatabaseSpec::from_live_database(&client, &MigrationScopeMatches::all_schemas())
            .await
            .map_err(|e| DatabaseError::BoxedError(Box::new(e)))?
            .value;

    let migrations = Migration::from_schemas(
        current_database_spec,
        &DatabaseSpec::new(vec![], vec![]),
        &MigrationScope::all_schemas(),
    );
    migrations
        .apply(database, true)
        .await
        .map_err(|e| DatabaseError::BoxedError(e.into()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::subsystem::PostgresCoreSubsystem;

    use super::*;
    use core_plugin_interface::{
        error::ModelSerializationError, serializable_system::SerializableSystem,
    };
    use stripmargin::StripMargin;

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn add_model() {
        assert_changes(
            "",
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    published: Boolean
                }
            }
            "#,
            vec![],
            vec![(
                r#"CREATE TABLE "concerts" (
                   |    "id" SERIAL PRIMARY KEY,
                   |    "title" TEXT NOT NULL,
                   |    "published" BOOLEAN NOT NULL
                   |);"#,
                false,
            )],
            vec![(
                r#"CREATE TABLE "concerts" (
                   |    "id" SERIAL PRIMARY KEY,
                   |    "title" TEXT NOT NULL,
                   |    "published" BOOLEAN NOT NULL
                   |);"#,
                false,
            )],
            vec![(r#"DROP TABLE "concerts" CASCADE;"#, true)],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn add_field() {
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    published: Boolean
                }
            }
            "#,
            vec![(
                r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL
                    |);"#,
                false,
            )],
            vec![(
                r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "published" BOOLEAN NOT NULL
                    |);"#,
                false,
            )],
            vec![(
                r#"ALTER TABLE "concerts" ADD "published" BOOLEAN NOT NULL;"#,
                false,
            )],
            vec![(r#"ALTER TABLE "concerts" DROP COLUMN "published";"#, true)],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn add_relation_and_related_model() {
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL
                    |);"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                        |    "id" SERIAL PRIMARY KEY,
                        |    "name" TEXT NOT NULL
                        |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"ALTER TABLE "concerts" ADD "venue_id" INT NOT NULL;"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
            ],
            vec![
                (r#"ALTER TABLE "concerts" DROP COLUMN "venue_id";"#, true),
                (r#"DROP TABLE "venues" CASCADE;"#, true),
            ],
        ).await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn add_relation_field() {
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"ALTER TABLE "concerts" ADD "venue_id" INT NOT NULL;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
            ],
            vec![(r#"ALTER TABLE "concerts" DROP COLUMN "venue_id";"#, true)],
        ).await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn update_relation_optionality() {
        // venue: Venue <-> venue: Venue?
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue?
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                ("ALTER TABLE \"concerts\" ADD CONSTRAINT \"concerts_venue_id_fk\" FOREIGN KEY (\"venue_id\") REFERENCES \"venues\";", false),
            ],
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
            ],
            vec![
                ("ALTER TABLE \"concerts\" ALTER COLUMN \"venue_id\" DROP NOT NULL;", false),
            ],
            vec![("ALTER TABLE \"concerts\" ALTER COLUMN \"venue_id\" SET NOT NULL;", false)],
        ).await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn add_indices() {
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    @index title: String
                    @index venue: Venue
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    @index name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
                ("CREATE INDEX \"concert_title_idx\" ON \"concerts\" (\"title\");", false),
                ("CREATE INDEX \"concert_venue_idx\" ON \"concerts\" (\"venue_id\");", false),
                ("CREATE INDEX \"venue_name_idx\" ON \"venues\" (\"name\");", false),
            ],
            vec![
                ("CREATE INDEX \"concert_title_idx\" ON \"concerts\" (\"title\");", false),
                ("CREATE INDEX \"concert_venue_idx\" ON \"concerts\" (\"venue_id\");", false),
                ("CREATE INDEX \"venue_name_idx\" ON \"venues\" (\"name\");", false),
            ],
            vec![
                ("DROP INDEX \"concert_title_idx\";", false),
                ("DROP INDEX \"concert_venue_idx\";", false),
                ("DROP INDEX \"venue_name_idx\";", false),
            ],

        ).await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn modify_multi_column_indices() {
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    @index title: String
                    @index venue: Venue
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    @index name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    @index("title", "title-venue") title: String
                    @index("venue", "title-venue") venue: Venue
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    @index("name") name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
                ("CREATE INDEX \"concert_title_idx\" ON \"concerts\" (\"title\");", false), 
                ("CREATE INDEX \"concert_venue_idx\" ON \"concerts\" (\"venue_id\");", false), 
                ("CREATE INDEX \"venue_name_idx\" ON \"venues\" (\"name\");", false)       
            ],
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
                ("CREATE INDEX \"title\" ON \"concerts\" (\"title\");", false), 
                ("CREATE INDEX \"title-venue\" ON \"concerts\" (\"title\", \"venue_id\");", false), 
                ("CREATE INDEX \"venue\" ON \"concerts\" (\"venue_id\");", false), 
                ("CREATE INDEX \"name\" ON \"venues\" (\"name\");", false)
            ],
            vec![
                ("DROP INDEX \"concert_title_idx\";", false), 
                ("DROP INDEX \"concert_venue_idx\";", false), 
                ("CREATE INDEX \"title\" ON \"concerts\" (\"title\");", false), 
                ("CREATE INDEX \"title-venue\" ON \"concerts\" (\"title\", \"venue_id\");", false), 
                ("CREATE INDEX \"venue\" ON \"concerts\" (\"venue_id\");", false), 
                ("DROP INDEX \"venue_name_idx\";", false), 
                ("CREATE INDEX \"name\" ON \"venues\" (\"name\");", false)
            ],
            vec![
                ("DROP INDEX \"title\";", false), 
                ("DROP INDEX \"title-venue\";", false), 
                ("DROP INDEX \"venue\";", false), 
                ("CREATE INDEX \"concert_title_idx\" ON \"concerts\" (\"title\");", false), 
                ("CREATE INDEX \"concert_venue_idx\" ON \"concerts\" (\"venue_id\");", false), 
                ("DROP INDEX \"name\";", false), 
                ("CREATE INDEX \"venue_name_idx\" ON \"venues\" (\"name\");", false)
            ],

        ).await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn add_indices_non_public_schemas() {
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue
                }

                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                @table(schema="c")
                type Concert {
                    @pk id: Int = autoIncrement()
                    @index title: String
                    @index venue: Venue
                }

                @table(schema="v")
                type Venue {
                    @pk id: Int = autoIncrement()
                    @index name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
            ],
            vec![
                ("CREATE SCHEMA \"c\";", false), 
                ("CREATE SCHEMA \"v\";", false),
                (
                    r#"CREATE TABLE "c"."concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "v"."venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "c"."concerts" ADD CONSTRAINT "c_concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "v"."venues";"#,
                    false,
                ),
                ("CREATE INDEX \"concert_title_idx\" ON \"c\".\"concerts\" (\"title\");", false),
                ("CREATE INDEX \"concert_venue_idx\" ON \"c\".\"concerts\" (\"venue_id\");", false),
                ("CREATE INDEX \"venue_name_idx\" ON \"v\".\"venues\" (\"name\");", false),
            ],
            vec![
                ("CREATE SCHEMA \"c\";", false), 
                ("CREATE SCHEMA \"v\";", false),
                ("DROP TABLE \"concerts\" CASCADE;", true), 
                ("DROP TABLE \"venues\" CASCADE;", true),
                (
                    r#"CREATE TABLE "c"."concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "v"."venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                ("ALTER TABLE \"c\".\"concerts\" ADD CONSTRAINT \"c_concerts_venue_id_fk\" FOREIGN KEY (\"venue_id\") REFERENCES \"v\".\"venues\";", false),
                ("CREATE INDEX \"concert_title_idx\" ON \"c\".\"concerts\" (\"title\");", false),
                ("CREATE INDEX \"concert_venue_idx\" ON \"c\".\"concerts\" (\"venue_id\");", false),
                ("CREATE INDEX \"venue_name_idx\" ON \"v\".\"venues\" (\"name\");", false),
            ],
            vec![
                ("DROP TABLE \"c\".\"concerts\" CASCADE;", true), 
                ("DROP TABLE \"v\".\"venues\" CASCADE;", true),
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                ("DROP SCHEMA \"c\" CASCADE;", true), 
                ("DROP SCHEMA \"v\" CASCADE;", true), 
                ("ALTER TABLE \"concerts\" ADD CONSTRAINT \"concerts_venue_id_fk\" FOREIGN KEY (\"venue_id\") REFERENCES \"venues\";", false)
            ],

        ).await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn one_to_one_constraints() {
        assert_changes(
            r#"
                @postgres
                module MembershipModule {
                    type Membership {
                        @pk id: Int = autoIncrement()
                    }
                    type User {
                        @pk id: Int = autoIncrement()
                        name: String
                    }
                }
            "#,
            r#"
                @postgres
                module MembershipModule {
                    type Membership {
                        @pk id: Int = autoIncrement()
                        user: User
                    }
                    type User {
                        @pk id: Int = autoIncrement()
                        name: String
                        membership: Membership?
                    }
                }
            "#,
            vec![
                (
                    r#"CREATE TABLE "memberships" (
                    |    "id" SERIAL PRIMARY KEY
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "users" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"CREATE TABLE "memberships" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "user_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "users" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "memberships" ADD CONSTRAINT "memberships_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "users";"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "memberships" ADD CONSTRAINT "unique_constraint_membership_user" UNIQUE ("user_id");"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"ALTER TABLE "memberships" ADD "user_id" INT NOT NULL;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "memberships" ADD CONSTRAINT "unique_constraint_membership_user" UNIQUE ("user_id");"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "memberships" ADD CONSTRAINT "memberships_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "users";"#,
                    false,
                ),
            ],
            vec![
                (r#"ALTER TABLE "memberships" DROP COLUMN "user_id";"#, true),
                (
                    r#"ALTER TABLE "memberships" DROP CONSTRAINT "unique_constraint_membership_user";"#,
                    false,
                ),
            ],
        ).await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn multi_column_unique_constraint() {
        assert_changes(
            r#"
                @postgres
                module RsvpModule {
                    type Rsvp {
                        @pk id: Int = autoIncrement()
                        email: String
                        event_id: Int
                    }
                }
            "#,
            r#"
                @postgres
                module RsvpModule {
                    type Rsvp {
                        @pk id: Int = autoIncrement()
                        @unique("email_event_id") email: String
                        @unique("email_event_id") event_id: Int
                    }
                }
            "#,
            vec![
                (
                    r#"CREATE TABLE "rsvps" (
                |    "id" SERIAL PRIMARY KEY,
                |    "email" TEXT NOT NULL,
                |    "event_id" INT NOT NULL
                |);"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"CREATE TABLE "rsvps" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "email" TEXT NOT NULL,
                    |    "event_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "rsvps" ADD CONSTRAINT "unique_constraint_rsvp_email_event_id" UNIQUE ("email", "event_id");"#,
                    false,
                ),
            ],
            vec![(
                r#"ALTER TABLE "rsvps" ADD CONSTRAINT "unique_constraint_rsvp_email_event_id" UNIQUE ("email", "event_id");"#,
                false,
            )],
            vec![(
                r#"ALTER TABLE "rsvps" DROP CONSTRAINT "unique_constraint_rsvp_email_event_id";"#,
                false,
            )],
        ).await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn multi_column_unique_constraint_participation_change() {
        assert_changes(
            r#"
                @postgres
                module RsvpModule {
                    type Rsvp {
                        @pk id: Int = autoIncrement()
                        @unique("email_event_id") email: String
                        event_id: Int
                    }
                }
            "#,
            r#"
                @postgres
                module RsvpModule {
                    type Rsvp {
                        @pk id: Int = autoIncrement()
                        @unique("email_event_id") email: String
                        @unique("email_event_id") event_id: Int
                    }
                }
            "#,
            vec![
                (
                    r#"CREATE TABLE "rsvps" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "email" TEXT NOT NULL,
                    |    "event_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "rsvps" ADD CONSTRAINT "unique_constraint_rsvp_email_event_id" UNIQUE ("email");"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"CREATE TABLE "rsvps" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "email" TEXT NOT NULL,
                    |    "event_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "rsvps" ADD CONSTRAINT "unique_constraint_rsvp_email_event_id" UNIQUE ("email", "event_id");"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"ALTER TABLE "rsvps" DROP CONSTRAINT "unique_constraint_rsvp_email_event_id";"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "rsvps" ADD CONSTRAINT "unique_constraint_rsvp_email_event_id" UNIQUE ("email", "event_id");"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"ALTER TABLE "rsvps" DROP CONSTRAINT "unique_constraint_rsvp_email_event_id";"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "rsvps" ADD CONSTRAINT "unique_constraint_rsvp_email_event_id" UNIQUE ("email");"#,
                    false,
                ),
            ],
        ).await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn default_value_change() {
        assert_changes(
            r#"
                @postgres
                module UserModule {
                    type User {
                        @pk id: Int = autoIncrement()
                        role: String
                        verified: Boolean = false
                        enabled: Boolean = true
                    }
                }
            "#,
            r#"
                @postgres
                module UserModule {
                    type User {
                        @pk id: Int = autoIncrement()
                        role: String = "USER" // Set default value
                        verified: Boolean = true // Change default value
                        enabled: Boolean // Drop default value
                    }
                }
            "#,
            vec![(
                r#"CREATE TABLE "users" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "role" TEXT NOT NULL,
                    |    "verified" BOOLEAN NOT NULL DEFAULT false,
                    |    "enabled" BOOLEAN NOT NULL DEFAULT true
                    |);"#,
                false,
            )],
            vec![(
                r#"CREATE TABLE "users" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "role" TEXT NOT NULL DEFAULT 'USER'::text,
                    |    "verified" BOOLEAN NOT NULL DEFAULT true,
                    |    "enabled" BOOLEAN NOT NULL
                    |);"#,
                false,
            )],
            vec![
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "role" SET DEFAULT 'USER'::text;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "verified" SET DEFAULT true;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "enabled" DROP DEFAULT;"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "role" DROP DEFAULT;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "verified" SET DEFAULT false;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "enabled" SET DEFAULT true;"#,
                    false,
                ),
            ],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn not_null() {
        assert_changes(
            r#"
                @postgres
                module LogModule {
                    type Log {
                        @pk id: Int
                        level: String?
                        message: String
                    }
                }
            "#,
            r#"
                @postgres
                module LogModule {
                    type Log {
                        @pk id: Int
                        level: String
                        message: String
                    }
                }
            "#,
            vec![(
                r#"CREATE TABLE "logs" (
                    |    "id" INT PRIMARY KEY,
                    |    "level" TEXT,
                    |    "message" TEXT NOT NULL
                    |);"#,
                false,
            )],
            vec![(
                r#"CREATE TABLE "logs" (
                    |    "id" INT PRIMARY KEY,
                    |    "level" TEXT NOT NULL,
                    |    "message" TEXT NOT NULL
                    |);"#,
                false,
            )],
            vec![(
                r#"ALTER TABLE "logs" ALTER COLUMN "level" SET NOT NULL;"#,
                false,
            )],
            vec![(
                r#"ALTER TABLE "logs" ALTER COLUMN "level" DROP NOT NULL;"#,
                false,
            )],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn non_public_schema() {
        assert_changes(
            r#"
                @postgres
                module LogModule {
                    type Log {
                        @pk id: Int
                        level: String?
                        message: String
                        owner: User
                    }

                    type User {
                        @pk id: Int
                        name: String
                        logs: Set<Log>?
                    }
                }
            "#,
            r#"
                @postgres
                module LogModule {
                    type Log {
                        @pk id: Int
                        level: String?
                        message: String
                        owner: User
                    }

                    @table(schema="auth")
                    type User {
                        @pk id: Int
                        name: String
                        logs: Set<Log>?
                    }
                }
            "#,
            vec![
                (
                    r#"CREATE TABLE "logs" (
                    |    "id" INT PRIMARY KEY,
                    |    "level" TEXT,
                    |    "message" TEXT NOT NULL,
                    |    "owner_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (r#"ALTER TABLE "logs" ADD CONSTRAINT "logs_owner_id_fk" FOREIGN KEY ("owner_id") REFERENCES "users";"#, false),
            ],
            vec![
                (r#"CREATE SCHEMA "auth";"#, false),
                (
                    r#"CREATE TABLE "logs" (
                    |    "id" INT PRIMARY KEY,
                    |    "level" TEXT,
                    |    "message" TEXT NOT NULL,
                    |    "owner_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "auth"."users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (r#"ALTER TABLE "logs" ADD CONSTRAINT "logs_owner_id_fk" FOREIGN KEY ("owner_id") REFERENCES "auth"."users";"#, false),
            ],
            vec![
                (r#"CREATE SCHEMA "auth";"#, false),
                (r#"DROP TABLE "users" CASCADE;"#, true),
                (
                    r#"CREATE TABLE "auth"."users" (
                 |    "id" INT PRIMARY KEY,
                 |    "name" TEXT NOT NULL
                 |);"#,
                    false,
                ),
            ],
            vec![
                (r#"DROP TABLE "auth"."users" CASCADE;"#, true),
                (
                    r#"CREATE TABLE "users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                ("DROP SCHEMA \"auth\" CASCADE;", true),
            ],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn non_public_module_level_schema() {
        assert_changes(
            r#"
                @postgres
                module LogModule {
                    type Log {
                        @pk id: Int
                        level: String?
                        message: String
                        owner: User
                    }

                    type User {
                        @pk id: Int
                        name: String
                        logs: Set<Log>?
                    }
                }
            "#,
            r#"
                @postgres(schema="info")
                module LogModule {
                    type Log {
                        @pk id: Int
                        level: String?
                        message: String
                        owner: User
                    }

                    type User {
                        @pk id: Int
                        name: String
                        logs: Set<Log>?
                    }
                }
            "#,
            vec![
                (
                    r#"CREATE TABLE "logs" (
                    |    "id" INT PRIMARY KEY,
                    |    "level" TEXT,
                    |    "message" TEXT NOT NULL,
                    |    "owner_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (r#"ALTER TABLE "logs" ADD CONSTRAINT "logs_owner_id_fk" FOREIGN KEY ("owner_id") REFERENCES "users";"#, false),
            ],
            vec![
                (r#"CREATE SCHEMA "info";"#, false),
                (
                    r#"CREATE TABLE "info"."logs" (
                    |    "id" INT PRIMARY KEY,
                    |    "level" TEXT,
                    |    "message" TEXT NOT NULL,
                    |    "owner_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "info"."users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (r#"ALTER TABLE "info"."logs" ADD CONSTRAINT "info_logs_owner_id_fk" FOREIGN KEY ("owner_id") REFERENCES "info"."users";"#, false),
            ],
            vec![
                (r#"CREATE SCHEMA "info";"#, false),
                (r#"DROP TABLE "logs" CASCADE;"#, true),
                (r#"DROP TABLE "users" CASCADE;"#, true),
                (
                    r#"CREATE TABLE "info"."logs" (
                    |    "id" INT PRIMARY KEY,
                    |    "level" TEXT,
                    |    "message" TEXT NOT NULL,
                    |    "owner_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "info"."users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (r#"ALTER TABLE "info"."logs" ADD CONSTRAINT "info_logs_owner_id_fk" FOREIGN KEY ("owner_id") REFERENCES "info"."users";"#, false),
            ],
            vec![
                (r#"DROP TABLE "info"."logs" CASCADE;"#, true),
                (r#"DROP TABLE "info"."users" CASCADE;"#, true),
                (
                    r#"CREATE TABLE "logs" (
                    |    "id" INT PRIMARY KEY,
                    |    "level" TEXT,
                    |    "message" TEXT NOT NULL,
                    |    "owner_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,

                ),
                ("DROP SCHEMA \"info\" CASCADE;", true),
                ("ALTER TABLE \"logs\" ADD CONSTRAINT \"logs_owner_id_fk\" FOREIGN KEY (\"owner_id\") REFERENCES \"users\";", false)
            ],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn introduce_vector_field() {
        assert_changes(
            r#"
            @postgres
            module DocumentDatabase {
              @access(true)
              type Document {
                @pk id: Int = autoIncrement()
                content: String
              }
            }
            "#,
            r#"
            @postgres
            module DocumentDatabase {
              @access(true)
              type Document {
                @pk id: Int = autoIncrement()
                content: String
                contentVector: Vector?
              }
            }
            "#,
            vec![(
                r#"CREATE TABLE "documents" (
                 |    "id" SERIAL PRIMARY KEY,
                 |    "content" TEXT NOT NULL
                 |);"#,
                false,
            )],
            vec![
                (r#"CREATE EXTENSION "vector";"#, false),
                (
                    r#"CREATE TABLE "documents" (
                 |    "id" SERIAL PRIMARY KEY,
                 |    "content" TEXT NOT NULL,
                 |    "content_vector" Vector(1536)
                 |);"#,
                    false,
                ),
            ],
            vec![
                ("CREATE EXTENSION \"vector\";", false),
                (
                    "ALTER TABLE \"documents\" ADD \"content_vector\" Vector(1536);",
                    false,
                ),
            ],
            vec![
                (
                    "ALTER TABLE \"documents\" DROP COLUMN \"content_vector\";",
                    true,
                ),
                ("DROP EXTENSION \"vector\";", true),
            ],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn vector_indexes_default_distance_function() {
        assert_changes(
            r#"
            @postgres
            module DocumentDatabase {
              @access(true)
              type Document {
                @pk id: Int = autoIncrement()
                title: String
                content: String
                @size(3) contentVector: Vector?
              }
            }
            "#,
            r#"
            @postgres
            module DocumentDatabase {
              @access(true)
              type Document {
                @pk id: Int = autoIncrement()
                title: String
                content: String
                @index @size(3) contentVector: Vector?
              }
            }
            "#,
            vec![
                (r#"CREATE EXTENSION "vector";"#, false), 
                (r#"CREATE TABLE "documents" (
                 |    "id" SERIAL PRIMARY KEY,
                 |    "title" TEXT NOT NULL,
                 |    "content" TEXT NOT NULL,
                 |    "content_vector" Vector(3)
                 |);"#, false)],
            vec![
                (r#"CREATE EXTENSION "vector";"#, false), 
                (r#"CREATE TABLE "documents" (
                 |    "id" SERIAL PRIMARY KEY,
                 |    "title" TEXT NOT NULL,
                 |    "content" TEXT NOT NULL,
                 |    "content_vector" Vector(3)
                 |);"#, false), 
                (r#"CREATE INDEX "document_contentvector_idx" ON "documents" USING hnsw ("content_vector" vector_cosine_ops);"#, false)
            ],
            vec![(r#"CREATE INDEX "document_contentvector_idx" ON "documents" USING hnsw ("content_vector" vector_cosine_ops);"#, false)],
            vec![(r#"DROP INDEX "document_contentvector_idx";"#, false)],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn vector_indexes_distance_function_change() {
        assert_changes(
            r#"
            @postgres
            module DocumentDatabase {
              @access(true)
              type Document {
                @pk id: Int = autoIncrement()
                title: String
                content: String
                @size(3) @index contentVector: Vector?
              }
            }
            "#,
            r#"
            @postgres
            module DocumentDatabase {
              @access(true)
              type Document {
                @pk id: Int = autoIncrement()
                title: String
                content: String
                @distanceFunction("l2") @index @size(3) contentVector: Vector?
              }
            }
            "#,
            vec![
                (r#"CREATE EXTENSION "vector";"#, false), 
                (r#"CREATE TABLE "documents" (
                 |    "id" SERIAL PRIMARY KEY,
                 |    "title" TEXT NOT NULL,
                 |    "content" TEXT NOT NULL,
                 |    "content_vector" Vector(3)
                 |);"#, false),
                 (r#"CREATE INDEX "document_contentvector_idx" ON "documents" USING hnsw ("content_vector" vector_cosine_ops);"#, false)
            ],
            vec![
                (r#"CREATE EXTENSION "vector";"#, false), 
                (r#"CREATE TABLE "documents" (
                 |    "id" SERIAL PRIMARY KEY,
                 |    "title" TEXT NOT NULL,
                 |    "content" TEXT NOT NULL,
                 |    "content_vector" Vector(3)
                 |);"#, false), 
                (r#"CREATE INDEX "document_contentvector_idx" ON "documents" USING hnsw ("content_vector" vector_l2_ops);"#, false)
            ],
            vec![
                (r#"DROP INDEX "document_contentvector_idx";"#, false), 
                (r#"CREATE INDEX "document_contentvector_idx" ON "documents" USING hnsw ("content_vector" vector_l2_ops);"#, false)],
            vec![
                (r#"DROP INDEX "document_contentvector_idx";"#, false), 
                (r#"CREATE INDEX "document_contentvector_idx" ON "documents" USING hnsw ("content_vector" vector_cosine_ops);"#, false)],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn vector_size_change() {
        assert_changes(
            r#"
            @postgres
            module DocumentDatabase {
              @access(true)
              type Document {
                @pk id: Int = autoIncrement()
                title: String
                content: String
                @size(3) contentVector: Vector?
              }
            }
            "#,
            r#"
            @postgres
            module DocumentDatabase {
              @access(true)
              type Document {
                @pk id: Int = autoIncrement()
                title: String
                content: String
                @size(4) contentVector: Vector?
              }
            }
            "#,
            vec![
                (r#"CREATE EXTENSION "vector";"#, false),
                (
                    r#"CREATE TABLE "documents" (
                 |    "id" SERIAL PRIMARY KEY,
                 |    "title" TEXT NOT NULL,
                 |    "content" TEXT NOT NULL,
                 |    "content_vector" Vector(3)
                 |);"#,
                    false,
                ),
            ],
            vec![
                (r#"CREATE EXTENSION "vector";"#, false),
                (
                    r#"CREATE TABLE "documents" (
                 |    "id" SERIAL PRIMARY KEY,
                 |    "title" TEXT NOT NULL,
                 |    "content" TEXT NOT NULL,
                 |    "content_vector" Vector(4)
                 |);"#,
                    false,
                ),
            ],
            vec![
                (
                    "ALTER TABLE \"documents\" DROP COLUMN \"content_vector\";",
                    true,
                ),
                (
                    "ALTER TABLE \"documents\" ADD \"content_vector\" Vector(4);",
                    false,
                ),
            ],
            vec![
                (
                    "ALTER TABLE \"documents\" DROP COLUMN \"content_vector\";",
                    true,
                ),
                (
                    "ALTER TABLE \"documents\" ADD \"content_vector\" Vector(3);",
                    false,
                ),
            ],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn add_update_sync_field() {
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    @update updatedAt: Instant = now()
                    @update modificationId: Uuid = generate_uuid()
                }
            }
            "#,
            vec![
                ("CREATE TABLE \"concerts\" (\n    \"id\" SERIAL PRIMARY KEY,\n    \"title\" TEXT NOT NULL\n);", false)
            ],
            vec![
                ("CREATE EXTENSION \"pgcrypto\";", false), 
                ("CREATE TABLE \"concerts\" (\n    \"id\" SERIAL PRIMARY KEY,\n    \"title\" TEXT NOT NULL,\n    \"updated_at\" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),\n    \"modification_id\" uuid NOT NULL DEFAULT gen_random_uuid()\n);", false),
                ("CREATE FUNCTION exograph_update_concerts() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = now(); NEW.modification_id = gen_random_uuid(); RETURN NEW; END; $$ language 'plpgsql';", false),
                ("CREATE TRIGGER exograph_on_update_concerts BEFORE UPDATE ON concerts FOR EACH ROW EXECUTE FUNCTION exograph_update_concerts();", false)
            ],
            vec![
                ("CREATE EXTENSION \"pgcrypto\";", false), 
                ("ALTER TABLE \"concerts\" ADD \"updated_at\" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now();", false),
                ("ALTER TABLE \"concerts\" ADD \"modification_id\" uuid NOT NULL DEFAULT gen_random_uuid();", false),
                ("CREATE FUNCTION exograph_update_concerts() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = now(); NEW.modification_id = gen_random_uuid(); RETURN NEW; END; $$ language 'plpgsql';", false),
                ("CREATE TRIGGER exograph_on_update_concerts BEFORE UPDATE ON concerts FOR EACH ROW EXECUTE FUNCTION exograph_update_concerts();", false)
            ],
            vec![
                ("ALTER TABLE \"concerts\" DROP COLUMN \"updated_at\";", true),
                ("ALTER TABLE \"concerts\" DROP COLUMN \"modification_id\";", true),
                ("DROP TRIGGER exograph_on_update_concerts on \"concerts\";", false),
                ("DROP FUNCTION exograph_update_concerts;", false),
                ("DROP EXTENSION \"pgcrypto\";", true)            
            ],
        ).await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn add_update_annotation() {
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    updatedAt: Instant = now()
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    @update updatedAt: Instant = now()
                }
            }
            "#,
            vec![
                ("CREATE TABLE \"concerts\" (\n    \"id\" SERIAL PRIMARY KEY,\n    \"title\" TEXT NOT NULL,\n    \"updated_at\" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()\n);", false)
            ],
            vec![
                ("CREATE TABLE \"concerts\" (\n    \"id\" SERIAL PRIMARY KEY,\n    \"title\" TEXT NOT NULL,\n    \"updated_at\" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()\n);", false),
                ("CREATE FUNCTION exograph_update_concerts() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = now(); RETURN NEW; END; $$ language 'plpgsql';", false),
                ("CREATE TRIGGER exograph_on_update_concerts BEFORE UPDATE ON concerts FOR EACH ROW EXECUTE FUNCTION exograph_update_concerts();", false)
            ],
            vec![
                ("CREATE FUNCTION exograph_update_concerts() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = now(); RETURN NEW; END; $$ language 'plpgsql';", false),
                ("CREATE TRIGGER exograph_on_update_concerts BEFORE UPDATE ON concerts FOR EACH ROW EXECUTE FUNCTION exograph_update_concerts();", false)
            ],
            vec![
                ("DROP TRIGGER exograph_on_update_concerts on \"concerts\";", false),
                ("DROP FUNCTION exograph_update_concerts;", false)          
            ],
        ).await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn public_named_schema_change_with_new_spec_scope() {
        assert_changes_with_scope(
            r#"
                @postgres
                module LogModule {
                    type User {
                        @pk id: Int
                        name: String
                    }
                }
            "#,
            r#"
                @postgres
                module LogModule {
                    @table(schema="auth")
                    type User {
                        @pk id: Int
                        name: String
                    }
                }
            "#,
            &MigrationScope::FromNewSpec,
            vec![(
                r#"CREATE TABLE "users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                false,
            )],
            vec![
                (r#"CREATE SCHEMA "auth";"#, false),
                (
                    r#"CREATE TABLE "auth"."users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
            ],
            vec![
                (r#"CREATE SCHEMA "auth";"#, false),
                (
                    r#"CREATE TABLE "auth"."users" (
                 |    "id" INT PRIMARY KEY,
                 |    "name" TEXT NOT NULL
                 |);"#,
                    false,
                ),
            ],
            vec![(
                r#"CREATE TABLE "users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                false,
            )],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn two_named_schema_change_with_new_spec_scope() {
        assert_changes_with_scope(
            r#"
                @postgres(schema="log")
                module LogModule {
                    type User {
                        @pk id: Int
                        name: String
                    }
                }
            "#,
            r#"
                @postgres
                module LogModule {
                    @table(schema="auth")
                    type User {
                        @pk id: Int
                        name: String
                    }
                }
            "#,
            &MigrationScope::FromNewSpec,
            vec![
                (r#"CREATE SCHEMA "log";"#, false),
                (
                    r#"CREATE TABLE "log"."users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
            ],
            vec![
                (r#"CREATE SCHEMA "auth";"#, false),
                (
                    r#"CREATE TABLE "auth"."users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
            ],
            vec![
                (r#"CREATE SCHEMA "auth";"#, false),
                (
                    r#"CREATE TABLE "auth"."users" (
                 |    "id" INT PRIMARY KEY,
                 |    "name" TEXT NOT NULL
                 |);"#,
                    false,
                ),
            ],
            vec![
                (r#"CREATE SCHEMA "log";"#, false),
                (
                    r#"CREATE TABLE "log"."users" (
                    |    "id" INT PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
            ],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn unmanaged_type_change() {
        assert_changes_with_scope(
            r#"
                @postgres
                module LogModule {
                    @table(managed=false)
                    type User {
                        @pk id: Int
                        name: String
                    }
                }
            "#,
            r#"
                @postgres
                module LogModule {
                    @table(managed=false)
                    type User {
                        @pk id: Int
                        name: String
                        email: String
                    }
                }
            "#,
            &MigrationScope::FromNewSpec,
            vec![],
            vec![],
            vec![],
            vec![],
        )
        .await
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn managed_to_unmanaged_type_change() {
        assert_changes_with_scope(
            r#"
                @postgres
                module LogModule {
                    type User {
                        @pk id: Int
                        name: String
                    }
                }
            "#,
            r#"
                @postgres
                module LogModule {
                    @table(managed=false)
                    type User {
                        @pk id: Int
                        name: String
                        email: String
                    }
                }
            "#,
            &MigrationScope::FromNewSpec,
            vec![("CREATE TABLE \"users\" (\n    \"id\" INT PRIMARY KEY,\n    \"name\" TEXT NOT NULL\n);", false)],
            vec![],
            vec![],
            vec![("ALTER TABLE \"users\" DROP COLUMN \"email\";", true)],
        )
        .await
    }

    async fn create_postgres_system_from_str(
        model_str: &str,
        file_name: String,
    ) -> Result<PostgresCoreSubsystem, ModelSerializationError> {
        let system = builder::build_system_from_str(
            model_str,
            file_name,
            vec![Box::new(
                postgres_builder::PostgresSubsystemBuilder::default(),
            )],
        )
        .await
        .unwrap();

        deserialize_postgres_subsystem(system)
    }

    fn deserialize_postgres_subsystem(
        system: SerializableSystem,
    ) -> Result<PostgresCoreSubsystem, ModelSerializationError> {
        let postgres_subsystem = system
            .subsystems
            .into_iter()
            .find(|subsystem| subsystem.id == "postgres");

        use core_plugin_interface::system_serializer::SystemSerializer;
        match postgres_subsystem {
            Some(subsystem) => {
                let postgres_core_subsystem = PostgresCoreSubsystem::deserialize(subsystem.core.0)?;
                Ok(postgres_core_subsystem)
            }
            None => Ok(PostgresCoreSubsystem::default()),
        }
    }

    async fn compute_spec(model: &str) -> DatabaseSpec {
        let postgres_core_subsystem =
            create_postgres_system_from_str(model, "test.exo".to_string())
                .await
                .unwrap();

        DatabaseSpec::from_database(&postgres_core_subsystem.database)
    }

    async fn assert_changes(
        old_system: &str,
        new_system: &str,
        old_create: Vec<(&str, bool)>,
        new_create: Vec<(&str, bool)>,
        up_migration: Vec<(&str, bool)>,
        down_migration: Vec<(&str, bool)>,
    ) {
        assert_changes_with_scope(
            old_system,
            new_system,
            &MigrationScope::all_schemas(),
            old_create,
            new_create,
            up_migration,
            down_migration,
        )
        .await;
    }

    async fn assert_changes_with_scope(
        old_system: &str,
        new_system: &str,
        scope: &MigrationScope,
        old_create: Vec<(&str, bool)>,
        new_create: Vec<(&str, bool)>,
        up_migration: Vec<(&str, bool)>,
        down_migration: Vec<(&str, bool)>,
    ) {
        let old_system = compute_spec(old_system).await;
        let new_system = compute_spec(new_system).await;

        assert_change_with_scope(
            &DatabaseSpec::new(vec![], vec![]),
            &old_system,
            scope,
            old_create,
            "Create old system schema",
        );
        assert_change_with_scope(
            &DatabaseSpec::new(vec![], vec![]),
            &new_system,
            scope,
            new_create,
            "Create new system schema",
        );

        // Check that migration is idempotent by checking that re-migrating yield no changes
        assert_change_with_scope(
            &old_system,
            &old_system,
            scope,
            vec![],
            "Idempotent with old model",
        );

        assert_change_with_scope(
            &new_system,
            &new_system,
            scope,
            vec![],
            "Idempotent with new model",
        );

        // Up changes old -> new
        assert_change_with_scope(
            &old_system,
            &new_system,
            scope,
            up_migration,
            "Up migration",
        );
        // Down changes new -> old
        assert_change_with_scope(
            &new_system,
            &old_system,
            scope,
            down_migration,
            "Down migration",
        );
    }

    fn assert_change_with_scope(
        old_system: &DatabaseSpec,
        new_system: &DatabaseSpec,
        scope: &MigrationScope,
        expected: Vec<(&str, bool)>,
        message: &str,
    ) {
        fn clean_actual(actual: Migration) -> Vec<(String, bool)> {
            actual
                .statements
                .into_iter()
                .map(
                    |MigrationStatement {
                         statement: s,
                         is_destructive: d,
                     }| (s.replace('\t', "    "), d),
                )
                .collect()
        }
        fn clean_expected(expected: Vec<(&str, bool)>) -> Vec<(String, bool)> {
            expected
                .into_iter()
                .map(|(s, d)| (s.strip_margin(), d))
                .collect()
        }

        let actual = Migration::from_schemas(old_system, new_system, scope);

        let actual_changes = clean_actual(actual);
        let expected_changes = clean_expected(expected);

        assert_eq!(actual_changes, expected_changes, "{message}");
    }
}
