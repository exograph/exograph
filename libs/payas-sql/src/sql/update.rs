use maybe_owned::MaybeOwned;

use crate::PhysicalTable;

use super::{
    column::{Column, PhysicalColumn, ProxyColumn},
    predicate::ConcretePredicate,
    transaction::{TransactionContext, TransactionStepId},
    Expression, SQLBuilder, SQLParamContainer,
};

#[derive(Debug)]
pub struct Update<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
    pub column_values: Vec<(&'a PhysicalColumn, MaybeOwned<'a, Column<'a>>)>,
    pub returning: Vec<MaybeOwned<'a, Column<'a>>>,
}

impl<'a> Expression for Update<'a> {
    fn binding(&self, builder: &mut SQLBuilder) {
        builder.push_str("UPDATE ");
        self.table.binding(builder);
        builder.push_str(" SET ");
        builder.push_iter(
            self.column_values.iter(),
            ", ",
            |builder, (column, value)| {
                builder.with_plain(|builder| {
                    column.binding(builder);
                });
                builder.push_str(" = ");
                value.binding(builder);
            },
        );

        if self.predicate.as_ref() != &ConcretePredicate::True {
            builder.push_str(" WHERE ");
            self.predicate.binding(builder);
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
                                MaybeOwned::Owned(Column::Literal(SQLParamContainer::new(
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
