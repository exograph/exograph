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
#[cfg(test)]
mod test_util;

pub mod array_util;
pub mod column;
pub mod database;
pub mod database_client;
pub mod order;
pub mod physical_column;
pub mod predicate;
pub mod relation;
pub mod vector;

pub use sql_bytes::SQLBytes;
pub use sql_param::SQLParam;
pub use sql_param_container::SQLParamContainer;

pub(crate) mod cte;
pub(crate) mod delete;
pub(crate) mod group_by;
pub(crate) mod insert;
pub(crate) mod join;
pub(crate) mod json_agg;
pub(crate) mod json_object;
pub(crate) mod limit;
pub(crate) mod offset;
pub(crate) mod physical_table;
pub(crate) mod select;
pub(crate) mod sql_operation;
pub(crate) mod table;
pub(crate) mod transaction;
pub(crate) mod update;

pub(crate) use expression_builder::ExpressionBuilder;
pub(crate) use sql_builder::SQLBuilder;
pub(crate) use sql_value::SQLValue;

mod expression_builder;

mod sql_builder;
mod sql_bytes;
mod sql_param;
mod sql_param_container;
mod sql_value;
