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
pub mod physical_column_type;
pub mod sql_bytes;
pub mod sql_param;
pub mod sql_param_container;
pub mod sql_value;

mod predicate_ext;

pub use exo_sql_core::operation::{CaseSensitivity, NumericComparator, ParamEquality, Predicate};
pub use function::Function;
pub use pg_extension::{PgExtension, VectorDistanceOperand};
#[cfg(any(test, feature = "test-support"))]
pub mod test_database_builder;

mod function_ext;
mod limit_ext;
mod offset_ext;
mod pg_column_type;
mod pg_extension_ext;
mod physical_column_ext;
mod physical_table_ext;

pub use pg_column_type::{PgColumnType, PgColumnTypeExt, as_pg_column_type, to_pg_array_type};

// Pg-specialized model type aliases
pub type PgAbstractOperation = exo_sql_model::AbstractOperation<PgExtension>;
pub type PgAbstractSelect = exo_sql_model::AbstractSelect<PgExtension>;
pub type PgAbstractInsert = exo_sql_model::AbstractInsert<PgExtension>;
pub type PgAbstractUpdate = exo_sql_model::AbstractUpdate<PgExtension>;
pub type PgAbstractDelete = exo_sql_model::AbstractDelete<PgExtension>;
pub type PgAbstractPredicate = exo_sql_model::AbstractPredicate<PgExtension>;
pub type PgAbstractOrderBy = exo_sql_model::AbstractOrderBy<PgExtension>;
pub type PgColumnPath = exo_sql_model::ColumnPath<PgExtension>;
pub type PgSelection = exo_sql_model::Selection<PgExtension>;
pub type PgSelectionElement = exo_sql_model::SelectionElement<PgExtension>;
pub type PgAliasedSelectionElement = exo_sql_model::AliasedSelectionElement<PgExtension>;
pub type PgInsertionRow = exo_sql_model::InsertionRow<PgExtension>;
pub type PgInsertionElement = exo_sql_model::InsertionElement<PgExtension>;
pub type PgNestedInsertion = exo_sql_model::NestedInsertion<PgExtension>;
pub type PgNestedAbstractUpdate = exo_sql_model::NestedAbstractUpdate<PgExtension>;
pub type PgNestedAbstractInsert = exo_sql_model::NestedAbstractInsert<PgExtension>;
pub type PgNestedAbstractInsertSet = exo_sql_model::NestedAbstractInsertSet<PgExtension>;
pub type PgNestedAbstractDelete = exo_sql_model::NestedAbstractDelete<PgExtension>;
pub type PgColumnValuePair = exo_sql_model::ColumnValuePair<PgExtension>;
pub use physical_column_type::ensure_registry_initialized;

pub use column::{Column, ProxyColumn};
pub use expression_builder::ExpressionBuilder;
pub use order::{OrderBy, OrderByElement, OrderByElementExpr};
pub use pg_extension::ArrayParamWrapper;
pub use predicate_ext::ConcretePredicate;
pub use sql_builder::SQLBuilder;
pub use sql_operation::{SQLOperation, TemplateSQLOperation};
pub use transaction::{
    ConcreteTransactionStep, DynamicTransactionStep, TemplateFilterOperation,
    TemplateTransactionStep, TransactionContext, TransactionScript, TransactionStep,
    TransactionStepId, TransactionStepResult,
};
pub use vector::VectorDistance;

#[cfg(feature = "bigdecimal")]
pub use pg_bigdecimal::BigDecimal;
