// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::ParamEquality;

/// Supertrait that all database extension types must satisfy.
/// Used as the bound on the `Ext` type parameter throughout the SQL AST types.
pub trait DatabaseExtension: std::fmt::Debug + PartialEq + ParamEquality + Clone {}

/// Blanket implementation so any type satisfying the bounds automatically implements DatabaseExtension.
impl<T: std::fmt::Debug + PartialEq + ParamEquality + Clone> DatabaseExtension for T {}
