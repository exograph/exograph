// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{Limit, Offset, PhysicalTable};

use super::{order_by::AbstractOrderBy, predicate::AbstractPredicate, selection::Selection};

/// Represents an abstract select operation, but without specific details about how to execute it.
#[derive(Debug)]
pub struct AbstractSelect<'a> {
    /// The table to select from
    pub table: &'a PhysicalTable,
    /// The columns to select
    pub selection: Selection<'a>,
    /// The predicate to filter rows. This is not an `Option` to ensure that the caller makes a conscious
    /// decision about whether to use `True` or `False` (rather than assuming that `None` means `True` or `False`).
    pub predicate: AbstractPredicate<'a>,
    /// The order by clause
    pub order_by: Option<AbstractOrderBy<'a>>,
    /// The offset
    pub offset: Option<Offset>,
    /// The limit
    pub limit: Option<Limit>,
}
