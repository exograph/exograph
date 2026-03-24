// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::operation::DatabaseExtension;
use exo_sql_core::{Limit, Offset, TableId};

use crate::{order_by::AbstractOrderBy, predicate::AbstractPredicate, selection::Selection};

/// Represents an abstract select operation, but without specific details about how to execute it.
#[derive(Debug)]
pub struct AbstractSelect<Ext: DatabaseExtension> {
    /// The table to select from
    pub table_id: TableId,
    /// The columns to select
    pub selection: Selection<Ext>,
    /// The predicate to filter rows. This is not an `Option` to ensure that the caller makes a conscious
    /// decision about whether to use `True` or `False` (rather than assuming that `None` means `True` or `False`).
    pub predicate: AbstractPredicate<Ext>,
    /// The order by clause
    pub order_by: Option<AbstractOrderBy<Ext>>,
    /// The offset
    pub offset: Option<Offset>,
    /// The limit
    pub limit: Option<Limit>,
}
