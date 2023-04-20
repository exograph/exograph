// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use maybe_owned::MaybeOwned;

use super::{
    column::Column, physical_table::PhysicalTable, predicate::ConcretePredicate, ExpressionBuilder,
    SQLBuilder,
};

/// A delete operation.
#[derive(Debug)]
pub struct Delete<'a> {
    /// The table to delete from.
    pub table: &'a PhysicalTable,
    /// The predicate to filter rows by.
    pub predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
    /// The columns to return.
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

impl<'a> ExpressionBuilder for Delete<'a> {
    /// Build a delete operation for the `DELETE FROM <table> WHERE <predicate> RETURNING <returning>`.
    /// The `WHERE` clause is omitted if the predicate is `true` and the `RETURNING` clause is omitted
    /// if the list of columns to return is empty.
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("DELETE FROM ");
        self.table.build(builder);

        if self.predicate.as_ref() != &ConcretePredicate::True {
            builder.push_str(" WHERE ");
            self.predicate.build(builder);
        }

        if !self.returning.is_empty() {
            builder.push_str(" RETURNING ");
            builder.push_elems(&self.returning, ", ");
        }
    }
}

#[derive(Debug)]
pub struct TemplateDelete<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: ConcretePredicate<'a>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

// TODO: Tie this properly to the prev_step
impl<'a> TemplateDelete<'a> {
    pub fn resolve(&'a self) -> Delete<'a> {
        let TemplateDelete {
            table,
            predicate,
            returning,
        } = self;

        Delete {
            table,
            predicate: predicate.into(),
            returning: returning
                .iter()
                .map(|c| MaybeOwned::Borrowed(c.as_ref()))
                .collect(),
        }
    }
}
