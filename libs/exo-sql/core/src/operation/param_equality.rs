// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

/// Compare two parameters so that we can reduce a predicate to a boolean before passing it to
/// the database. With a simpler form, we may be able to skip passing it to the database completely. For
/// example, `Predicate::Eq(Column::Param(1), Column::Param(1))` can be reduced to
/// true.
pub trait ParamEquality {
    /// Returns `None` if one of the columns is not a parameter, otherwise returns `Some(true)` if
    /// the parameters are equal, and `Some(false)` if they are not.
    fn param_eq(&self, other: &Self) -> Option<bool>;
}

/// Default implementation for the unit type (used as the default `Ext` parameter).
impl ParamEquality for () {
    fn param_eq(&self, _other: &Self) -> Option<bool> {
        None
    }
}
