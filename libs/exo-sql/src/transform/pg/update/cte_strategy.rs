use maybe_owned::MaybeOwned;

use crate::{
    sql::{
        cte::{CteExpression, WithQuery},
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::{
        pg::{selection_level::SelectionLevel, Postgres},
        transformer::{PredicateTransformer, SelectTransformer},
    },
    AbstractUpdate, Column, Database, PhysicalColumn,
};

use super::update_strategy::UpdateStrategy;

pub(crate) struct CteStrategy {}

// Suitable for simpler case where we do not have any nested insert/update/delete. We can just return a simple
// CTE like:
// ```sql
// WITH "concerts" AS (
//    UPDATE "concerts" SET "title" = $1 WHERE "concerts"."id" = $2 RETURNING *
// )
// SELECT json_build_object('id', "concerts"."id")::text FROM "concerts" WHERE "concerts"."id" = $3
// ```
impl UpdateStrategy for CteStrategy {
    fn id(&self) -> &'static str {
        "CteStrategy"
    }

    fn suitable(&self, abstract_update: &AbstractUpdate, _database: &Database) -> bool {
        abstract_update.nested_updates.is_empty()
            && abstract_update.nested_inserts.is_empty()
            && abstract_update.nested_deletes.is_empty()
    }

    fn update_transaction_script<'a>(
        &self,
        abstract_update: &'a AbstractUpdate,
        database: &'a Database,
        transformer: &Postgres,
        transaction_script: &mut TransactionScript<'a>,
    ) {
        let table = database.get_table(abstract_update.table_id);

        let column_values: Vec<(&'a PhysicalColumn, MaybeOwned<'a, Column>)> = abstract_update
            .column_values
            .iter()
            .map(|(col_id, v)| (col_id.get_column(database), v.into()))
            .collect();

        let predicate = transformer.to_predicate(
            &abstract_update.predicate,
            &SelectionLevel::TopLevel,
            false,
            database,
        );

        let root_update = SQLOperation::Update(table.update(
            column_values,
            predicate.into(),
            vec![Column::Star(None).into()],
        ));

        let select = transformer.to_select(&abstract_update.selection, database);

        let table_name = table.name.clone();

        transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
            SQLOperation::WithQuery(WithQuery {
                expressions: vec![CteExpression {
                    name: table_name,
                    table_name: None,
                    operation: root_update,
                }],
                select,
            }),
        )));
    }
}
