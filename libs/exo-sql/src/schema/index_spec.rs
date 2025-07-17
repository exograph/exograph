// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    SchemaObjectName, VectorDistanceFunction, database_error::DatabaseError,
    sql::connect::database_client::DatabaseClient,
};

use super::{
    DebugPrintTo, column_spec::ColumnSpec, issue::WithIssues, op::SchemaOp, table_spec::TableSpec,
};

#[derive(Debug, Clone)]
pub struct IndexSpec {
    pub name: String,
    pub columns: HashSet<String>,
    pub index_kind: IndexKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum IndexKind {
    HNWS {
        distance_function: VectorDistanceFunction,
        params: Option<HNWSParams>,
    },
    #[default]
    DatabaseDefault,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct HNWSParams {
    pub m: usize,
    pub ef_construction: usize,
}

const INDICES_QUERY: &str = r#"
SELECT
   schema.nspname AS schema,
   index_info.indrelid :: regclass :: text AS table,
   array_agg(attribute.attname) as column_names,
   index_info.indisunique AS is_unique,
   cls.relname AS index_name,
   access_method.amname AS index_method,
   opc.operator_classes as index_opclasses
FROM
   pg_catalog.pg_namespace schema
   JOIN pg_catalog.pg_class cls ON cls.relnamespace = schema.oid
   JOIN pg_catalog.pg_index index_info ON index_info.indexrelid = cls.oid
   JOIN pg_catalog.pg_am access_method ON access_method.oid = cls.relam
   JOIN pg_attribute attribute ON attribute.attrelid = index_info.indrelid
   CROSS JOIN LATERAL (
      SELECT
         ARRAY (
            SELECT
               opc.opcname
            FROM
               unnest(index_info.indclass::oid[]) WITH ORDINALITY o(oid, ord)
               JOIN pg_opclass opc ON opc.oid = o.oid
         )
   ) opc(operator_classes)
WHERE
   index_info.indrelid :: regclass :: text = $1
   AND cls.relkind = 'i'
   AND attribute.attnum = ANY(index_info.indkey)
GROUP BY
   schema.nspname,
   index_info.indrelid,
   cls.relname,
   access_method.amname,
   opc.operator_classes,
   index_info.indisunique;
"#;

impl IndexSpec {
    pub fn new(name: String, columns: HashSet<String>, index_kind: IndexKind) -> Self {
        Self {
            name,
            columns,
            index_kind,
        }
    }

    // Does the other index serve the same purpose as this index?
    // Effectively, match ignoring the index name
    pub fn effectively_eq(&self, other: &IndexSpec) -> bool {
        self.columns == other.columns && self.index_kind == other.index_kind
    }

    pub async fn from_live_db(
        client: &DatabaseClient,
        table_name: &SchemaObjectName,
        columns: &[ColumnSpec],
    ) -> Result<WithIssues<Vec<IndexSpec>>, DatabaseError> {
        let indices = client
            .query(INDICES_QUERY, &[&table_name.fully_qualified_name()])
            .await?
            .iter()
            .flat_map(|row| {
                let column_names = row
                    .get::<_, Vec<String>>("column_names")
                    .into_iter()
                    .collect::<HashSet<_>>();
                let unique: bool = row.get("is_unique");

                // If the columns consists only of primary key columns, then we should not
                // explicitly create an index for it (the database will create it automatically due
                // to the pk constraint)
                if unique
                    || column_names
                        .iter()
                        .all(|c| columns.iter().any(|col| col.name == *c && col.is_pk))
                {
                    Ok::<_, DatabaseError>(None)
                } else {
                    let index_kind =
                        match row.get::<_, String>("index_method").to_lowercase().as_str() {
                            "hnsw" => {
                                let operator_classes: Vec<String> =
                                    row.get::<_, Vec<String>>("index_opclasses");
                                let distance_function =
                                    VectorDistanceFunction::from_db_string(&operator_classes[0])?;

                                Ok::<_, DatabaseError>(IndexKind::HNWS {
                                    distance_function,
                                    params: None,
                                })
                            }
                            _ => Ok(IndexKind::default()),
                        }?;
                    Ok(Some(IndexSpec::new(
                        row.get("index_name"),
                        column_names,
                        index_kind,
                    )))
                }
            })
            .flatten()
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
        // Ignore the index name, since other information is sufficient to define the index (the name is auxiliary to work with multiple columns)
        if self.columns == other.columns
            && self_table.name == other_table.name
            && self.index_kind == other.index_kind
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

    pub fn creation_sql(&self, table_name: &SchemaObjectName) -> String {
        let sorted_columns = {
            let mut columns = self.columns.iter().collect::<Vec<_>>();
            columns.sort();
            columns
        };

        let columns_str = sorted_columns
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");

        let index_spec_str = match &self.index_kind {
            IndexKind::HNWS {
                distance_function,
                params,
            } => {
                assert!(
                    self.columns.len() == 1,
                    "Vector index must have exactly one column"
                );

                let distance_function_str = distance_function.index_kind_str();
                let params_str = params
                    .as_ref()
                    .map(|p| {
                        format!(
                            " WITH (e = {}, ef_construction = {})",
                            p.m, p.ef_construction
                        )
                    })
                    .unwrap_or_else(|| "".to_string());
                format!("USING hnsw ({columns_str} {distance_function_str}){params_str}")
            }
            _ => format!("({columns_str})"),
        };

        format!(
            "CREATE INDEX \"{index_name}\" ON {table_name} {index_spec_str};",
            index_name = self.name,
            table_name = table_name.sql_name(),
        )
    }
}

impl DebugPrintTo for IndexSpec {
    fn debug_print_to<W: std::io::Write>(
        &self,
        writer: &mut W,
        indent: usize,
    ) -> std::io::Result<()> {
        let indent_str = " ".repeat(indent);
        let mut columns_sorted: Vec<_> = self.columns.iter().map(String::as_str).collect();
        columns_sorted.sort_unstable();
        let columns_str = columns_sorted.join(", ");

        let index_type = match &self.index_kind {
            IndexKind::HNWS {
                distance_function, ..
            } => format!("HNWS({:?})", distance_function),
            IndexKind::DatabaseDefault => "DEFAULT".to_string(),
        };

        writeln!(
            writer,
            "{}- ({}, {}, [{}])",
            indent_str, self.name, index_type, columns_str
        )
    }
}
