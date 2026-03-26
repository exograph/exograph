// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Low-level SQL primitives with a few PostgreSQL-specific extensions (json_agg, json_object, etc.)
//! We form these primitives from the higher-level SQL operations from the `asql` module. Primitives
//! such as `Select` and `Insert` are used to execute the corresponding SQL operations on the
//! database.

#[macro_use]
mod test_util;

pub mod array_util;
pub mod column;
pub mod cte;
pub mod delete;
pub mod expression_builder;
pub mod group_by;
pub mod insert;
pub mod join;
pub mod json_agg;
pub mod json_object;
pub mod order;
pub mod select;
pub mod sql_builder;
pub mod sql_operation;
pub mod table;
pub mod transaction;
pub mod update;
pub mod vector;

pub mod function;
pub mod pg_extension;
pub mod pg_schema_types;
pub mod physical_column_type;
pub mod sql_bytes;
pub mod sql_param;
pub mod sql_param_container;
pub mod sql_value;

mod predicate_expr;
pub(crate) use predicate_expr::ConcretePredicate;

#[cfg(any(test, feature = "test-support"))]
pub mod test_database_builder;

mod function_expr;
mod limit_expr;
mod offset_expr;
mod pg_column_type;
pub use pg_column_type::PgColumnTypeExt;
mod column_extension_expr;
mod physical_column_expr;
mod physical_table_expr;
