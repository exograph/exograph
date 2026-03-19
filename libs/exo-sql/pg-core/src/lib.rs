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
pub mod predicate;
pub mod select;
pub mod sql_builder;
pub mod sql_operation;
pub mod table;
pub mod transaction;
pub mod update;
pub mod vector;

mod function_ext;
mod limit_ext;
mod offset_ext;
mod physical_column_ext;
mod physical_table_ext;

pub use column::{ArrayParamWrapper, Column, ProxyColumn};
pub use expression_builder::ExpressionBuilder;
pub use order::{OrderBy, OrderByElement, OrderByElementExpr, VectorDistanceOperand};
pub use predicate::ConcretePredicate;
pub use sql_builder::SQLBuilder;
pub use sql_operation::{SQLOperation, TemplateSQLOperation};
pub use transaction::{
    ConcreteTransactionStep, DynamicTransactionStep, TemplateFilterOperation,
    TemplateTransactionStep, TransactionContext, TransactionScript, TransactionStep,
    TransactionStepId, TransactionStepResult,
};
pub use vector::VectorDistance;
