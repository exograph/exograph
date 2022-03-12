use crate::sql::{
    column::Column,
    cte::Cte,
    predicate::Predicate,
    sql_operation::SQLOperation,
    transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    PhysicalTable,
};

use super::{
    predicate::AbstractPredicate,
    select::{AbstractSelect, SelectionLevel},
};

#[derive(Debug)]
pub struct AbstractDelete<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: Option<AbstractPredicate<'a>>,
    pub selection: AbstractSelect<'a>,
}

impl<'a> AbstractDelete<'a> {
    pub(crate) fn to_transaction_script(
        self,
        additional_predicate: Option<Predicate<'a>>,
    ) -> TransactionScript<'a> {
        // TODO: Consider the "join" aspect of the predicate
        let predicate = Predicate::and(
            self.predicate
                .map(|p| p.predicate())
                .unwrap_or_else(|| Predicate::True),
            additional_predicate.unwrap_or(Predicate::True),
        );

        let root_delete = SQLOperation::Delete(
            self.table
                .delete(predicate.into(), vec![Column::Star.into()]),
        );
        let select = self.selection.to_select(None, SelectionLevel::TopLevel);

        let mut transaction_script = TransactionScript::default();

        transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
            SQLOperation::Cte(Cte {
                ctes: vec![(self.table.name.clone(), root_delete)],
                select,
            }),
        )));

        transaction_script
    }
}
