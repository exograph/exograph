// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::DatabaseExtension;
use super::predicate::ColumnPredicate;
use super::table::Table;

/// Represents a join between two tables. Currently, supports only left join.
#[derive(Debug, PartialEq, Clone)]
pub struct LeftJoin<Ext: DatabaseExtension> {
    /// The left table in the join such as `concerts`.
    left: Box<Table<Ext>>,
    /// The right table in the join such as `venues`.
    right: Box<Table<Ext>>,
    /// The join predicate such as `concerts.venue_id = venues.id`.
    predicate: ColumnPredicate<Ext>,
}

impl<Ext: DatabaseExtension> LeftJoin<Ext> {
    pub fn new(left: Table<Ext>, right: Table<Ext>, predicate: ColumnPredicate<Ext>) -> Self {
        LeftJoin {
            left: Box::new(left),
            right: Box::new(right),
            predicate,
        }
    }

    pub fn left(&self) -> &Table<Ext> {
        &self.left
    }

    pub fn right(&self) -> &Table<Ext> {
        &self.right
    }

    pub fn predicate(&self) -> &ColumnPredicate<Ext> {
        &self.predicate
    }
}
