use maybe_owned::MaybeOwned;

use crate::PhysicalTable;

use super::{
    column::{Column, ProxyColumn},
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
    pub predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
    /// The columns to update and their values.
    pub column_values: Vec<(&'a PhysicalColumn, MaybeOwned<'a, Column<'a>>)>,
    /// The columns to return.
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

impl<'a> ExpressionBuilder for Update<'a> {
    /// Build the update statement for the form `UPDATE <table> SET <column = value, ...> WHERE
    /// <predicate> RETURNING <returning-columns>`. The `WHERE` is omitted if the predicate is
    /// `True` and `RETURNING` is omitted if the list of columns to return is empty.
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("UPDATE ");
        self.table.build(builder);

        builder.push_str(" SET ");
        builder.push_iter(
            self.column_values.iter(),
            ", ",
            |builder, (column, value)| {
                builder.without_fully_qualified_column_names(|builder| {
                    column.build(builder);
                });

                builder.push_str(" = ");

                value.build(builder);
            },
        );

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
pub struct TemplateUpdate<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: ConcretePredicate<'a>,
    pub column_values: Vec<(&'a PhysicalColumn, ProxyColumn<'a>)>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

impl<'a> TemplateUpdate<'a> {
    pub fn resolve(
        &'a self,
        prev_step_id: TransactionStepId,
        transaction_context: &TransactionContext,
    ) -> Vec<Update<'a>> {
        let rows = transaction_context.row_count(prev_step_id);

        let TemplateUpdate {
            table,
            predicate,
            column_values,
            returning,
        } = self;

        (0..rows)
            .map(|row_index| {
                let resolved_column_values = column_values
                    .iter()
                    .map(|(physical_col, col)| {
                        let resolved_col = match col {
                            ProxyColumn::Concrete(col) => col.as_ref().into(),
                            ProxyColumn::Template { col_index, step_id } => {
                                MaybeOwned::Owned(Column::Param(SQLParamContainer::new(
                                    transaction_context
                                        .resolve_value(*step_id, row_index, *col_index),
                                )))
                            }
                        };
                        (*physical_col, resolved_col)
                    })
                    .collect();
                Update {
                    table,
                    predicate: predicate.into(),
                    column_values: resolved_column_values,
                    returning: returning.iter().map(|col| col.as_ref().into()).collect(),
                }
            })
            .collect()
    }
}
