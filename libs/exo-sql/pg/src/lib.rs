// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Postgres backend for exo-sql.
//!
//! - `core`: SQL primitives, types, rendering, parameter binding
//! - `transform`: abstract-to-concrete SQL transformation

#[macro_use]
pub mod core;
pub mod transform;

pub use core::array_util;
pub use core::column;
pub use core::physical_column_type;
pub use core::sql_param_container;
pub use core::transaction;

#[cfg(any(test, feature = "test-support"))]
pub use core::test_database_builder;

// Re-export transform module as `pg` (used by pg-connect and internal transform code)
pub use transform as pg;

// Re-export commonly used types
pub use core::function::Function;
pub use core::pg_column_type::PgColumnTypeExt;
pub use core::pg_extension::PgExtension;
pub use core::physical_column_type::ensure_registry_initialized;
pub use exo_sql_core::operation::{CaseSensitivity, NumericComparator, ParamEquality, Predicate};

// Pg-specialized model type aliases
pub type PgAbstractOperation = exo_sql_model::AbstractOperation<PgExtension>;
pub type PgAbstractSelect = exo_sql_model::AbstractSelect<PgExtension>;
pub type PgAbstractInsert = exo_sql_model::AbstractInsert<PgExtension>;
pub type PgAbstractUpdate = exo_sql_model::AbstractUpdate<PgExtension>;
pub type PgAbstractDelete = exo_sql_model::AbstractDelete<PgExtension>;
pub type PgAbstractPredicate = exo_sql_model::AbstractPredicate<PgExtension>;
pub type PgAbstractOrderBy = exo_sql_model::AbstractOrderBy<PgExtension>;
pub type PgColumnPath = exo_sql_model::ColumnPath<PgExtension>;
pub(crate) type PgSelection = exo_sql_model::Selection<PgExtension>;
pub type PgSelectionElement = exo_sql_model::SelectionElement<PgExtension>;
pub type PgAliasedSelectionElement = exo_sql_model::AliasedSelectionElement<PgExtension>;
pub type PgInsertionRow = exo_sql_model::InsertionRow<PgExtension>;
pub type PgInsertionElement = exo_sql_model::InsertionElement<PgExtension>;
pub(crate) type PgNestedInsertion = exo_sql_model::NestedInsertion<PgExtension>;
pub type PgNestedAbstractUpdate = exo_sql_model::NestedAbstractUpdate<PgExtension>;
pub type PgNestedAbstractInsert = exo_sql_model::NestedAbstractInsert<PgExtension>;
pub type PgNestedAbstractInsertSet = exo_sql_model::NestedAbstractInsertSet<PgExtension>;
pub type PgNestedAbstractDelete = exo_sql_model::NestedAbstractDelete<PgExtension>;
pub(crate) type PgColumnValuePair = exo_sql_model::ColumnValuePair<PgExtension>;

pub use core::column::Column;
pub use core::transaction::{TransactionScript, TransactionStepResult};

// Types used by pg-connect
pub use core::expression_builder::ExpressionBuilder;
pub use core::sql_builder::SQLBuilder;
pub use core::sql_operation::SQLOperation;

#[cfg(feature = "bigdecimal")]
pub use pg_bigdecimal::BigDecimal;
