// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use deadpool_postgres::Client;

use crate::{database_error::DatabaseError, PhysicalTableName};

use super::{column_spec::ColumnSpec, issue::WithIssues, op::SchemaOp, table_spec::TableSpec};

#[derive(Debug, Clone)]
pub struct IndexSpec {
    pub name: String,
    pub columns: HashSet<String>,
    pub is_unique: bool,
}

impl IndexSpec {
    pub fn new(name: String, columns: HashSet<String>, is_unique: bool) -> Self {
        Self {
            name,
            columns,
            is_unique,
        }
    }

    pub async fn from_live_db(
        client: &Client,
        table_name: &PhysicalTableName,
        columns: &[ColumnSpec],
    ) -> Result<WithIssues<Vec<IndexSpec>>, DatabaseError> {
        let indices_query = r#"
            SELECT tables.relname as table_name, indices.relname as index_name, array_agg(attr.attname) as column_names, index_info.indisunique as is_unique
            FROM pg_class tables, pg_class indices, pg_index index_info, pg_attribute attr
            WHERE
                tables.oid = index_info.indrelid 
                AND indices.oid = index_info.indexrelid
                AND attr.attrelid = tables.oid
                AND attr.attnum = ANY(index_info.indkey)
                AND tables.relkind = 'r'
                AND tables.relname = $1 
            GROUP BY tables.relname, indices.relname, index_info.indisunique"#;

        let indices = client
            .query(indices_query, &[&table_name.name.as_str()])
            .await?
            .iter()
            .flat_map(|row| {
                let column_names = row
                    .get::<_, Vec<String>>("column_names")
                    .into_iter()
                    .collect::<HashSet<_>>();

                // If the columns consists only of primary key columns, then we should not
                // explicitly create an index for it (the database will create it automatically due
                // to the pk constraint)
                if column_names
                    .iter()
                    .all(|c| columns.iter().any(|col| col.name == *c && col.is_pk))
                {
                    None
                } else {
                    Some(IndexSpec::new(
                        row.get("index_name"),
                        column_names,
                        row.get("is_unique"),
                    ))
                }
            })
            .collect::<Vec<_>>();

        Ok(WithIssues {
            value: indices,
            issues: vec![],
        })
    }

    pub fn diff<'a>(
        &'a self,
        other: &'a IndexSpec,
        self_table: &'a TableSpec,
        other_table: &'a TableSpec,
    ) -> Vec<SchemaOp<'a>> {
        if self.name == other.name
            && self.columns == other.columns
            && self.is_unique == other.is_unique
            && self_table.name == other_table.name
        {
            return vec![];
        }

        vec![
            SchemaOp::DeleteIndex {
                index: self,
                table: self_table,
            },
            SchemaOp::CreateIndex {
                index: other,
                table: other_table,
            },
        ]
    }

    pub fn creation_sql(&self, table_name: &PhysicalTableName) -> String {
        let sorted_columns = {
            let mut columns = self.columns.iter().collect::<Vec<_>>();
            columns.sort();
            columns
        };

        format!(
            "CREATE {unique}INDEX \"{index_name}\" ON {table_name} ({columns});",
            unique = if self.is_unique { "UNIQUE " } else { "" },
            index_name = self.name,
            table_name = table_name.sql_name(),
            columns = sorted_columns
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
