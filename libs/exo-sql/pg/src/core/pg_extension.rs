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
//! `PgExtension` is the unified extension type for all Postgres-specific AST variants
//! that don't belong in database-agnostic types. Column, Function, and OrderByElementExpr
//! use `Extension(PgExtension)` for their Postgres-specific variants.

use exo_sql_core::operation::{DatabaseExtension, ParamEquality};
use exo_sql_core::{VectorDistanceFunction, physical_column::ColumnId};

use crate::json_agg::JsonAgg;
use crate::json_object::JsonObject;
use crate::sql_param_container::SQLParamContainer;

/// Postgres-specific extensions to the generic SQL AST types.
#[derive(Debug, PartialEq, Clone)]
pub enum PgExtension {
    // -- Column extensions --
    /// An array parameter with a wrapping such as ANY() or ALL()
    ArrayParam {
        param: SQLParamContainer,
        wrapper: ArrayParamWrapper,
    },
    /// A JSON object (`json_build_object(...)`)
    JsonObject(JsonObject),
    /// A JSON array aggregation (`json_agg(...)`)
    JsonAgg(JsonAgg),

    // -- Function extensions --
    /// Vector distance function (pgvector)
    VectorDistanceFunction {
        column_id: ColumnId,
        distance_function: VectorDistanceFunction,
        target: SQLParamContainer,
    },

    // -- OrderBy extensions --
    /// Vector distance ordering (pgvector)
    VectorDistance(
        VectorDistanceOperand,
        VectorDistanceOperand,
        VectorDistanceFunction,
    ),
}

impl DatabaseExtension for PgExtension {
    type Param = SQLParamContainer;
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

impl ParamEquality for PgExtension {
    fn param_eq(&self, _other: &Self) -> Option<bool> {
        None
    }
}
