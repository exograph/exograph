// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{SchemaObjectName, TableId};

use super::DatabaseExtension;
use super::join::LeftJoin;
use super::select::Select;

/// A table-like concept that can be used in place of `SELECT FROM <table-query> ...`.
#[derive(Debug, PartialEq, Clone)]
pub enum Table<Ext: DatabaseExtension = ()> {
    /// A physical table such as `concerts`.
    Physical {
        table_id: TableId,
        alias: Option<String>,
    },
    /// A join between two tables such as `concerts LEFT JOIN venues ON concerts.venue_id = venues.id`.
    Join(Box<LeftJoin<Ext>>),
    /// A sub-select such as `(SELECT * FROM concerts) AS concerts`.
    SubSelect {
        select: Box<Select<Ext>>,
        /// The alias of the sub-select (optional, since we need to alias the sub-select when used in a FROM clause)
        alias: Option<(String, SchemaObjectName)>,
    },
}

impl<Ext: DatabaseExtension> Table<Ext> {
    pub fn physical(table_id: TableId, alias: Option<String>) -> Self {
        Table::Physical { table_id, alias }
    }
}
