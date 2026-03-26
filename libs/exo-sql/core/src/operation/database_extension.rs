// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt::Debug;

use crate::PhysicalColumnPath;

use super::ParamEquality;

/// Trait for predicate extensions to report their column paths.
pub trait PredicateExtensionPaths<C> {
    fn column_paths(&self) -> Vec<&C>;
}

/// Trait for order-by extensions to report their physical column paths.
pub trait AbstractOrderByExtensionPaths {
    fn physical_column_paths(&self) -> Vec<&PhysicalColumnPath>;
}

/// Supertrait that all database extension types must satisfy.
/// Used as the bound on the `Ext` type parameter throughout the SQL AST types.
pub trait DatabaseExtension: Debug + PartialEq + Clone {
    /// The type used for parameterized query values (e.g., `$1`, `?`).
    /// Parameters are a universal SQL concept; only the rendering format is backend-specific.
    type Param: Debug + PartialEq + Clone;

    /// Database-specific column extensions (e.g., array params, JSON aggregation).
    type ColumnExtension: Debug + PartialEq + ParamEquality + Clone;

    /// Database-specific function extensions (e.g., pgvector distance functions).
    type FunctionExtension: Debug + PartialEq + Clone;

    /// Database-specific order-by extensions (e.g., pgvector distance ordering).
    type OrderByExtension: Debug + PartialEq + Clone;

    /// Database-specific predicate extensions (e.g., pgvector distance predicates).
    type PredicateExtension<C: Debug + PartialEq + ParamEquality + Clone>: Debug
        + PartialEq
        + Clone
        + PredicateExtensionPaths<C>;

    /// Database-specific abstract order-by extensions (e.g., pgvector distance ordering).
    type AbstractOrderByExtension: Debug + AbstractOrderByExtensionPaths;
}
