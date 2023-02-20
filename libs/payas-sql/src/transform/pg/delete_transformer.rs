use tracing::instrument;

use crate::{
    asql::{delete::AbstractDelete, select::SelectionLevel},
    sql::{
        column::Column,
        cte::Cte,
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::{
        join_util,
        table_dependency::TableDependency,
        transformer::{DeleteTransformer, SelectTransformer},
    },
    ColumnPath, ColumnPathLink,
};

use super::Postgres;

impl DeleteTransformer for Postgres {
    #[instrument(
        name = "DeleteTransformer::to_transaction_script for Postgres"
        skip(self)
        )]
    fn to_transaction_script<'a>(
        &self,
        abstract_delete: &'a AbstractDelete,
    ) -> TransactionScript<'a> {
        // TODO: De-dump from the select transformer before committing
        fn column_path_owned<'a>(
            column_paths: Vec<&ColumnPath<'a>>,
        ) -> Vec<Vec<ColumnPathLink<'a>>> {
            column_paths
                .into_iter()
                .filter_map(|path| match path {
                    ColumnPath::Physical(links) => Some(links.to_vec()),
                    _ => None,
                })
                .collect()
        }

        // TODO: Consider the "join" aspect of the predicate
        let predicate_column_paths: Vec<Vec<ColumnPathLink>> =
            column_path_owned(abstract_delete.predicate.column_paths());

        let dependencies = TableDependency::from_column_path(predicate_column_paths);

        println!(
            "dependencies: {:#?}",
            &dependencies.as_ref().map(|d| &d.dependencies),
        );

        // DELETE FROM "concert_artists" WHERE "concerts"."id" = $1 RETURNING *
        // DELETE FROM "concert_artists" WHERE "concert_artists"."concert_id" in (select "concerts"."id" from "concerts" where "concerts"."id" = $1) RETURNING *;
        let predicate = abstract_delete.predicate.predicate();

        let root_delete = SQLOperation::Delete(
            abstract_delete
                .table
                .delete(predicate.into(), vec![Column::Star(None).into()]),
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
