// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Postgres-specific extensions to the generic SQL AST types.
//!
//! Each extension point has its own dedicated type:
//! - `PgColumnExtension` for `Column::Extension`
//! - `PgFunctionExtension` for `Function::Extension`
//! - `PgOrderByExtension` for `OrderByElementExpr::Extension`
//!
//! `PgExtension` is a unit struct that ties them together via `DatabaseExtension`.

use std::fmt::Debug;

use exo_sql_core::column_path::PhysicalColumnPath;
use exo_sql_core::operation::{
    AbstractOrderByExtensionPaths, DatabaseExtension, ParamEquality, PredicateExtensionPaths,
};
use exo_sql_core::physical_column::ColumnId;

use crate::core::json_agg::JsonAgg;
use crate::core::json_object::JsonObject;
use crate::core::vector::VectorDistanceFunction;
use crate::sql_param_container::SQLParamContainer;

use exo_sql_model::ColumnPath;

/// Postgres database extension marker type.
///
/// Ties together all Postgres-specific extension types via `DatabaseExtension`.
#[derive(PartialEq, Clone)]
pub struct PgExtension;

impl std::fmt::Debug for PgExtension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PgExtension")
    }
}

/// Postgres-specific column extensions.
#[derive(Debug, PartialEq, Clone)]
pub enum PgColumnExtension {
    /// An array parameter with a wrapping such as ANY() or ALL()
    ArrayParam {
        param: SQLParamContainer,
        wrapper: ArrayParamWrapper,
    },
    /// A JSON object (`json_build_object(...)`)
    JsonObject(JsonObject),
    /// A JSON array aggregation (`json_agg(...)`)
    JsonAgg(JsonAgg),
}

/// Postgres-specific function extensions.
#[derive(Debug, PartialEq, Clone)]
pub enum PgFunctionExtension {
    /// pgvector distance function (e.g., `column <=> target::vector`)
    VectorDistance {
        column_id: ColumnId,
        distance_function: VectorDistanceFunction,
        target: SQLParamContainer,
    },
}

/// Postgres-specific order-by extensions.
#[derive(Debug, PartialEq, Clone)]
pub enum PgOrderByExtension {
    /// pgvector distance ordering
    VectorDistance(
        VectorDistanceOperand,
        VectorDistanceOperand,
        VectorDistanceFunction,
    ),
}

/// Postgres-specific predicate extensions.
#[derive(Debug, PartialEq, Clone)]
pub enum PgPredicateExtension<C> {
    VectorDistance {
        lhs: C,
        rhs: C,
        distance_function: VectorDistanceFunction,
        comparator: exo_sql_core::operation::NumericComparator,
        threshold: C,
    },
}

impl<C: Debug + PartialEq + ParamEquality + Clone> PredicateExtensionPaths<C>
    for PgPredicateExtension<C>
{
    fn column_paths(&self) -> Vec<&C> {
        match self {
            PgPredicateExtension::VectorDistance {
                lhs,
                rhs,
                threshold,
                ..
            } => vec![lhs, rhs, threshold],
        }
    }
}

/// Postgres-specific abstract order-by extensions.
#[derive(Debug)]
pub enum PgAbstractOrderByExtension {
    /// pgvector distance ordering
    VectorDistance {
        lhs: PgColumnPath,
        rhs: PgColumnPath,
        distance_function: VectorDistanceFunction,
    },
}

type PgColumnPath = ColumnPath<PgExtension>;

impl AbstractOrderByExtensionPaths for PgAbstractOrderByExtension {
    fn physical_column_paths(&self) -> Vec<&PhysicalColumnPath> {
        match self {
            PgAbstractOrderByExtension::VectorDistance { lhs, rhs, .. } => [lhs, rhs]
                .iter()
                .filter_map(|path| match path {
                    ColumnPath::Physical(path) => Some(path),
                    _ => None,
                })
                .collect(),
        }
    }
}

impl DatabaseExtension for PgExtension {
    type Param = SQLParamContainer;
    type ColumnExtension = PgColumnExtension;
    type FunctionExtension = PgFunctionExtension;
    type OrderByExtension = PgOrderByExtension;

    type PredicateExtension<C: Debug + PartialEq + ParamEquality + Clone> = PgPredicateExtension<C>;

    type AbstractOrderByExtension = PgAbstractOrderByExtension;
}

/// The wrapper type for array parameters (e.g., `ANY($1)` or `ALL($1)`)
#[derive(Debug, PartialEq, Clone)]
pub enum ArrayParamWrapper {
    Any,
    All,
    None,
}

/// Operand for vector distance computation in ORDER BY.
#[derive(Debug, PartialEq, Clone)]
pub enum VectorDistanceOperand {
    PhysicalColumn(ColumnId),
    Param(SQLParamContainer),
}

impl ParamEquality for PgColumnExtension {
    fn param_eq(&self, _other: &Self) -> Option<bool> {
        None
    }
}
