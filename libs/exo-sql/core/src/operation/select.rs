// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{Limit, Offset};

use super::DatabaseExtension;
use super::Predicate;
use super::column::Column;
use super::group_by::GroupBy;
use super::order::OrderBy;
use super::table::Table;

/// A select statement
#[derive(Debug, PartialEq, Clone)]
pub struct Select<Ext: DatabaseExtension> {
    /// The table to select from
    pub table: Table<Ext>,
    /// The columns to select
    pub columns: Vec<Column<Ext>>,
    /// The predicate to filter the rows
    pub predicate: Predicate<Column<Ext>>,
    /// The order by clause
    pub order_by: Option<OrderBy<Ext>>,
    /// The offset clause
    pub offset: Option<Offset>,
    /// The limit clause
    pub limit: Option<Limit>,
    /// The group by clause
    pub group_by: Option<GroupBy>,
    /// Whether this is a top-level selection. This is used to put the `::text` cast on a top-level select statement.
    /// This way, we can grab the JSON as a string and return it to the user as is.
    pub top_level_selection: bool,
}
