// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::TableId;
use exo_sql_core::operation::DatabaseExtension;

use crate::{predicate::AbstractPredicate, select::AbstractSelect};

/// Abstract representation of a delete operation
#[derive(Debug)]
pub struct AbstractDelete<Ext: DatabaseExtension> {
    /// The table to delete from
    pub table_id: TableId,
    /// The predicate to filter rows.
    pub predicate: AbstractPredicate<Ext>,
    /// The selection to return
    pub selection: AbstractSelect<Ext>,

    /// The precheck predicates to run before deleting
    pub precheck_predicates: Vec<AbstractPredicate<Ext>>,
}
