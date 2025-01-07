// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use maybe_owned::MaybeOwned;

use crate::{Database, OneToMany, PhysicalTable};

use super::{
    column::Column,
    physical_column::PhysicalColumn,
    predicate::ConcretePredicate,
    transaction::{TransactionContext, TransactionStepId},
    ExpressionBuilder, SQLBuilder, SQLParamContainer,
};

/// An update operation.
#[derive(Debug)]
pub struct Update<'a> {
    /// The table to update.
    pub table: &'a PhysicalTable,
    /// The predicate to filter rows to update.
    pub predicate: MaybeOwned<'a, ConcretePredicate>,
    // Any additional predicate to filter rows to update.
    // TODO: Figure out a way to combine this with predicate (currently can't due to how we require combining predicates to take ownership of the constituent predicates)
    pub additional_predicate: Option<ConcretePredicate>,
    /// The columns to update and their values.
    pub column_values: Vec<(&'a PhysicalColumn, MaybeOwned<'a, Column>)>,
    /// The columns to return.
    pub returning: Vec<MaybeOwned<'a, Column>>,
}

impl<'a> ExpressionBuilder for Update<'a> {
    /// Build the update statement for the form `UPDATE <table> SET <column = value, ...> WHERE
    /// <predicate> RETURNING <returning-columns>`. The `WHERE` is omitted if the predicate is
    /// `True` and `RETURNING` is omitted if the list of columns to return is empty.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("UPDATE ");
        self.table.build(database, builder);

        builder.push_str(" SET ");
        builder.push_iter(
            self.column_values.iter(),
            ", ",
            |builder, (column, value)| {
                builder.without_fully_qualified_column_names(|builder| {
                    column.build(database, builder);
                });

                builder.push_str(" = ");

                value.build(database, builder);
            },
        );

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
pub struct TemplateUpdate<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: ConcretePredicate,
    pub nesting_relation: OneToMany,
    pub column_values: Vec<(&'a PhysicalColumn, &'a Column)>,
    pub returning: Vec<Column>,
}

impl<'a> TemplateUpdate<'a> {
    // Create a concrete update from the template version. Will examine the previous step's result
    // to create as many concrete operations as there are rows in the previous step.
    pub fn resolve(
        &'a self,
        prev_step_id: TransactionStepId,
        transaction_context: &TransactionContext,
    ) -> Vec<Update<'a>> {
        let rows = transaction_context.row_count(prev_step_id);

        // Go over all the rows in the previous step and create a concrete update for each row.
        (0..rows)
            .map(|row_index| {
                let resolved_column_values = self
                    .column_values
                    .iter()
                    .map(|(physical_col, col)| {
                        let resolved_col = (*col).into();
                        (*physical_col, resolved_col)
                    })
                    .collect();

                let relation_predicate = ConcretePredicate::Eq(
                    Column::Physical {
                        column_id: self.nesting_relation.column_pairs[0].foreign_column_id,
                        table_alias: None,
                    },
                    Column::Param(SQLParamContainer::from_sql_value(
                        transaction_context.resolve_value(prev_step_id, row_index, 0),
                    )),
                );

                Update {
                    table: self.table,
                    predicate: (&self.predicate).into(),
                    additional_predicate: Some(relation_predicate),
                    column_values: resolved_column_values,
                    returning: self.returning.iter().map(|col| col.into()).collect(),
                }
            })
            .collect()
    }
}
