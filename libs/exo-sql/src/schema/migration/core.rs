// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt::Display;

use crate::{
    Database, DatabaseClient,
    database_error::DatabaseError,
    schema::{
        database_spec::DatabaseSpec,
        issue::WithIssues,
        op::SchemaOp,
        spec::{MigrationScope, MigrationScopeMatches, diff},
    },
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

#[derive(Debug)]
pub enum VerificationErrors {
    PostgresError(DatabaseError),
    ModelNotCompatible(Vec<String>),
}

impl std::error::Error for VerificationErrors {}

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

#[derive(thiserror::Error, Debug)]
pub enum MigrationError {
    #[error("Generic error: {0}")]
    Generic(String),

    #[error("Postgres error: {0}")]
    Postgres(#[from] tokio_postgres::Error),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Error: {0}")]
    Error(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl Migration {
    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }

    pub fn from_schemas(
        old_schema_spec: &DatabaseSpec,
        new_schema_spec: &DatabaseSpec,
        scope: &MigrationScope,
    ) -> Self {
        let diffs = diff(old_schema_spec, new_schema_spec, scope);

        let diffs = diffs
            .into_iter()
            .map(|diff| (diff, None))
            .collect::<Vec<_>>();

        Self::from_diffs(&diffs)
    }

    pub fn from_diffs(diffs: &[(SchemaOp, Option<bool>)]) -> Self {
        let mut pre_statements = vec![];
        let mut statements = vec![];
        let mut post_statements = vec![];

        for (diff, is_destructive_override) in diffs.iter() {
            let is_destructive = is_destructive_override.unwrap_or(diff.is_destructive());

            let statement = diff.to_sql();

            for constraint in statement.pre_statements.into_iter() {
                if !constraint.trim().is_empty() {
                    pre_statements.push(MigrationStatement::new(constraint, is_destructive));
                }
            }

            if !statement.statement.trim().is_empty() {
                statements.push(MigrationStatement::new(statement.statement, is_destructive));
            }

            for constraint in statement.post_statements.into_iter() {
                if !constraint.trim().is_empty() {
                    post_statements.push(MigrationStatement::new(constraint, is_destructive));
                }
            }
        }

        pre_statements.extend(statements);
        pre_statements.extend(post_statements);

        Migration {
            statements: pre_statements,
        }
    }

    pub async fn extract_schema_from_db(
        client: &DatabaseClient,
        database_spec: &DatabaseSpec,
        scope: &MigrationScope,
    ) -> Result<WithIssues<DatabaseSpec>, DatabaseError> {
        let scope_matches = match scope {
            MigrationScope::Specified(scope) => scope,
            MigrationScope::FromNewSpec => {
                &MigrationScopeMatches::from_specs_schemas(&[database_spec])
            }
        };

        extract_db_schema(client, scope_matches).await
    }

    pub async fn from_db_and_model(
        client: &DatabaseClient,
        database: &Database,
        scope: &MigrationScope,
    ) -> Result<Self, DatabaseError> {
        let new_spec = DatabaseSpec::from_database(database);

        let old_schema = Self::extract_schema_from_db(client, &new_spec, scope).await?;

        for issue in &old_schema.issues {
            eprintln!("{issue}");
        }

        Ok(Migration::from_schemas(&old_schema.value, &new_spec, scope))
    }

    pub fn has_destructive_changes(&self) -> bool {
        self.statements
            .iter()
            .any(|statement| statement.is_destructive)
    }

    pub async fn verify(
        client: &DatabaseClient,
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
        client: &mut DatabaseClient,
        allow_destructive_changes: bool,
    ) -> Result<(), MigrationError> {
        let transaction = client.transaction().await?;
        for MigrationStatement {
            statement,
            is_destructive,
        } in self.statements.iter()
        {
            if !is_destructive || allow_destructive_changes {
                transaction.execute(statement, &[]).await?;
            } else {
                return Err(MigrationError::Generic(format!(
                    "Destructive change detected: {}",
                    statement
                )));
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
    client: &DatabaseClient,
    scope: &MigrationScopeMatches,
) -> Result<WithIssues<DatabaseSpec>, DatabaseError> {
    DatabaseSpec::from_live_database(client, scope).await
}

pub async fn wipe_database(client: &mut DatabaseClient) -> Result<(), DatabaseError> {
    // wiping is equivalent to migrating to an empty database and deals with non-public schemas correctly
    let current_database_spec =
        &DatabaseSpec::from_live_database(client, &MigrationScopeMatches::all_schemas())
            .await
            .map_err(|e| DatabaseError::BoxedError(Box::new(e)))?
            .value;

    let migrations = Migration::from_schemas(
        current_database_spec,
        &DatabaseSpec::new(vec![], vec![], vec![]),
        &MigrationScope::all_schemas(),
    );
    migrations
        .apply(client, true)
        .await
        .map_err(|e| DatabaseError::BoxedError(e.into()))?;

    Ok(())
}
