use crate::{
    asql::{delete::AbstractDelete, select::SelectionLevel},
    sql::{
        column::Column,
        cte::Cte,
        predicate::Predicate,
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::transformer::{DeleteTransformer, SelectTransformer},
};

use super::Postgres;

impl DeleteTransformer for Postgres {
    fn to_transaction_script<'a>(
        &self,
        abstract_delete: &'a AbstractDelete,
        additional_predicate: Option<Predicate<'a>>,
    ) -> TransactionScript<'a> {
        // TODO: Consider the "join" aspect of the predicate
        let predicate = Predicate::and(
            abstract_delete
                .predicate
                .as_ref()
                .map(|p| p.predicate())
                .unwrap_or_else(|| Predicate::True),
            additional_predicate.unwrap_or(Predicate::True),
        );

        let root_delete = SQLOperation::Delete(
            abstract_delete
                .table
                .delete(predicate.into(), vec![Column::Star.into()]),
        );
        let select = self.to_select(&abstract_delete.selection, None, SelectionLevel::TopLevel);

        let mut transaction_script = TransactionScript::default();

        transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
            SQLOperation::Cte(Cte {
                ctes: vec![(abstract_delete.table.name.clone(), root_delete)],
                select,
            }),
        )));

        transaction_script
    }
}
