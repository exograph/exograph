// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::database_error::DatabaseError;
use crate::sql::connect::database_client::DatabaseClient;
use crate::PhysicalTableName;

use super::issue::WithIssues;
use super::op::SchemaOp;
use super::statement::SchemaStatement;

const ENUM_VARIANTS_QUERY: &str = "SELECT e.enumlabel AS enum_value FROM pg_type t JOIN pg_enum e ON t.oid = e.enumtypid JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace
  WHERE n.nspname = $1 AND t.typname = $2 ORDER BY e.enumsortorder;";

#[derive(Debug)]
pub struct EnumSpec {
    pub name: PhysicalTableName,
    pub variants: Vec<String>,
}

impl EnumSpec {
    pub fn new(name: PhysicalTableName, variants: Vec<String>) -> Self {
        Self { name, variants }
    }

    pub fn sql_name(&self) -> String {
        self.name.sql_name()
    }

    pub(super) async fn from_live_db_enum(
        client: &DatabaseClient,
        name: PhysicalTableName,
    ) -> Result<WithIssues<EnumSpec>, DatabaseError> {
        let mut variants = Vec::new();

        for row in client
            .query(ENUM_VARIANTS_QUERY, &[&name.schema_name(), &name.name])
            .await?
        {
            variants.push(row.get("enum_value"));
        }

        let issues = Vec::new();

        Ok(WithIssues {
            value: EnumSpec { name, variants },
            issues,
        })
    }

    pub fn diff<'a>(&'a self, new: &'a Self) -> Vec<SchemaOp<'a>> {
        let mut changes = Vec::new();

        let existing_variants = &self.variants;
        let new_variants = &new.variants;

        if existing_variants != new_variants {
            changes.push(SchemaOp::CreateEnum { enum_: self });
        }

        changes
    }

    /// Converts the table specification to SQL statements.
    pub(super) fn creation_sql(&self) -> SchemaStatement {
        let table_name = self.sql_name();

        let variants = self
            .variants
            .iter()
            .map(|v| format!("'{}'", v))
            .collect::<Vec<String>>()
            .join(", ");

        SchemaStatement {
            statement: format!("CREATE TYPE {table_name} AS ENUM ({variants});",),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }

    pub(super) fn deletion_sql(&self) -> SchemaStatement {
        SchemaStatement {
            statement: format!("DROP TYPE {} CASCADE;", self.sql_name()),
            pre_statements: vec![],
            post_statements: vec![],
        }
    }
}
