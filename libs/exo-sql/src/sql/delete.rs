// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use maybe_owned::MaybeOwned;

use crate::{Database, OneToMany, SQLParamContainer};

use super::{
    column::Column,
    physical_table::PhysicalTable,
    predicate::ConcretePredicate,
    transaction::{TransactionContext, TransactionStepId},
    ExpressionBuilder, SQLBuilder,
};

/// A delete operation.
#[derive(Debug)]
pub struct Delete<'a> {
    /// The table to delete from.
    pub table: &'a PhysicalTable,
    /// The predicate to filter rows by.
    pub predicate: MaybeOwned<'a, ConcretePredicate>,
    // Any additional predicate to filter rows to delete.
    // TODO: Figure out a way to combine this with predicate (currently can't due to how we require combining predicates to take ownership of the constituent predicates)
    pub additional_predicate: Option<ConcretePredicate>,
    /// The columns to return.
    pub returning: Vec<MaybeOwned<'a, Column>>,
}

impl<'a> ExpressionBuilder for Delete<'a> {
    /// Build a delete operation for the `DELETE FROM <table> WHERE <predicate> RETURNING <returning>`.
    /// The `WHERE` clause is omitted if the predicate is `true` and the `RETURNING` clause is omitted
    /// if the list of columns to return is empty.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("DELETE FROM ");
        self.table.build(database, builder);

        if self.predicate.as_ref() != &ConcretePredicate::True {
            builder.push_str(" WHERE ");
            self.predicate.build(database, builder);
        }

        if let Some(additional_predicate) = &self.additional_predicate {
            // If we have a predicate, we need to add an `AND` before the additional predicate, else we need to add a `WHERE`
            if self.predicate.as_ref() != &ConcretePredicate::True {
                builder.push_str(" AND ");
            } else {
                builder.push_str(" WHERE ");
            }
            additional_predicate.build(database, builder);
        }

        if !self.returning.is_empty() {
            builder.push_str(" RETURNING ");
            builder.push_elems(database, &self.returning, ", ");
        }
    }
}

#[derive(Debug)]
pub struct TemplateDelete<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: ConcretePredicate,
    pub nesting_relation: OneToMany,
    pub returning: Vec<Column>,
}

impl<'a> TemplateDelete<'a> {
    pub fn resolve(
        &'a self,
        prev_step_id: TransactionStepId,
        transaction_context: &TransactionContext,
    ) -> Vec<Delete<'a>> {
        let TemplateDelete {
            table,
            predicate,
            nesting_relation,
            returning,
        } = self;

        let rows = transaction_context.row_count(prev_step_id);

        // Go over all the rows in the previous step and create a concrete update for each row.
        (0..rows)
            .map(|row_index| {
                let relation_predicate = ConcretePredicate::Eq(
                    Column::Physical {
                        column_id: nesting_relation.column_pairs[0].foreign_column_id,
                        table_alias: None,
                    },
                    Column::Param(SQLParamContainer::from_sql_value(
                        transaction_context.resolve_value(prev_step_id, row_index, 0),
                    )),
                );

                Delete {
                    table,
                    predicate: predicate.into(),
                    additional_predicate: Some(relation_predicate),
                    returning: returning.iter().map(MaybeOwned::Borrowed).collect(),
                }
            })
            .collect()
    }
}
