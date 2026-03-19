// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Schema module re-exporting from exo-sql-pg-schema and exo-sql-core.

// Wrapper modules that merge types from both pg-schema and core
pub mod column_spec;
pub mod index_spec;
pub mod statement;

// Direct re-exports from pg-schema
pub use exo_sql_pg_schema::{
    database_spec, enum_spec, function_spec, issue, migration, op, spec, table_spec, trigger_spec,
};

// Re-export DebugPrintTo trait
pub use exo_sql_pg_schema::DebugPrintTo;
