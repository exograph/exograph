use maybe_owned::MaybeOwned;

use crate::PgAbstractUpdate;
use crate::{
    Column, SQLOperation,
    cte::{CteExpression, WithQuery},
    transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    update::Update,
};
use exo_sql_core::{Database, PhysicalColumn};
use exo_sql_model::{
    selection_level::SelectionLevel,
    transformer::{PredicateTransformer, SelectTransformer},
};

use crate::pg::{Postgres, precheck::add_precheck_queries};

use super::update_strategy::UpdateStrategy;

pub(crate) struct CteStrategy {}

// Suitable for a simpler case without any nested insert/update/delete. For nested cases, trying to
// build a CTE will result in a query that is too complex for Postgres to handle and still can't
// handle complex cases like recursively nested updates. In those cases, we fall back to the
// `super::multi_statement_strategy::MultiStatementStrategy`.
//
// Here we just return a simple CTE like:
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

    fn suitable(&self, abstract_update: &PgAbstractUpdate, _database: &Database) -> bool {
        abstract_update.nested_updates.is_empty()
            && abstract_update.nested_inserts.is_empty()
            && abstract_update.nested_deletes.is_empty()
    }

    fn update_transaction_script<'a>(
        &self,
        abstract_update: PgAbstractUpdate,
        database: &'a Database,
        transformer: &Postgres,
        transaction_script: &mut TransactionScript<'a>,
    ) {
        add_precheck_queries(
            abstract_update.precheck_predicates,
            database,
            transformer,
            transaction_script,
        );

        let table = database.get_table(abstract_update.table_id);

        let column_values: Vec<(&'a PhysicalColumn, MaybeOwned<'a, Column>)> = abstract_update
            .column_values
            .into_iter()
            .map(|(col_id, v)| (col_id.get_column(database), v.into()))
            .collect();

        let predicate = transformer.to_predicate(
            &abstract_update.predicate,
            &SelectionLevel::TopLevel,
            false,
            database,
        );

        let root_update = SQLOperation::Update(Update {
            table,
            column_values: column_values.into_iter().collect(),
            predicate: predicate.into(),
            additional_predicate: None,
            returning: vec![Column::Star(None).into()],
        });

        let select = transformer.to_select(abstract_update.selection, database);

        let table_name = &database.get_table(abstract_update.table_id).name;

        transaction_script.add_step(TransactionStep::Concrete(Box::new(
            ConcreteTransactionStep::new(SQLOperation::WithQuery(WithQuery {
                expressions: vec![CteExpression::new_auto_name(table_name, root_update)],
                select,
            })),
        )));
    }
}
