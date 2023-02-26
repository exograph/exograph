use tracing::instrument;

use crate::{
    asql::{delete::AbstractDelete, select::SelectionLevel},
    sql::{
        column::Column,
        cte::Cte,
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::transformer::{DeleteTransformer, SelectTransformer},
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
        let delete = self.to_delete(abstract_delete);
        let mut transaction_script = TransactionScript::default();
        transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
            SQLOperation::Cte(delete),
        )));
        transaction_script
    }

    /// Ignore the selection (instead returns a `*` and relies on to_transaction_script to use a CTE to do the selection).
    /// This way, we can do nested selection if needed.
    #[instrument(name = "DeleteTransformer::to_delete for Postgres", skip(self))]
    fn to_delete<'a>(&self, abstract_delete: &'a AbstractDelete) -> Cte<'a> {
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
        // let predicate_column_paths: Vec<Vec<ColumnPathLink>> =
        //     column_path_owned(abstract_delete.predicate.column_paths());
        // println!("predicate_column_paths: {predicate_column_paths:#?}");

        // let dependencies = TableDependency::from_column_path(predicate_column_paths);

        // println!("predicate: {:#?}", abstract_delete.predicate);
        // println!("dependencies: {dependencies:#?}");

        // DELETE FROM "concert_artists" WHERE "concerts"."id" = $1 RETURNING *
        // DELETE FROM "concert_artists" WHERE "concert_artists"."concert_id" in (select "concerts"."id" from "concerts" where "concerts"."id" = $1) RETURNING *;

        // current:  DELETE FROM "concerts" WHERE "venues"."name" = $1 RETURNING *
        // expected: DELETE FROM "concerts" WHERE "concerts"."venue_id" in (select "venues"."id" from "venues" where "venues"."name" = $1) RETURNING *

        let predicate = abstract_delete.predicate.predicate();
        let predicate_x = abstract_delete.predicate.predicate_x(&Postgres {});

        println!("predicate_x: {predicate_x:#?}");

        let root_delete = SQLOperation::Delete(
            abstract_delete
                .table
                .delete(predicate.into(), vec![Column::Star(None).into()]),
        );

        let select = self.to_select(&abstract_delete.selection, None, SelectionLevel::TopLevel);

        Cte {
            ctes: vec![(abstract_delete.table.name.clone(), root_delete)],
            select,
        }
    }
}

#[cfg(test)]
mod tests {
    use maybe_owned::MaybeOwned;

    use crate::{
        asql::selection::{ColumnSelection, Selection, SelectionElement},
        sql::{Expression, ExpressionContext},
        transform::{pg::Postgres, test_util::TestSetup},
        AbstractPredicate, AbstractSelect, Predicate,
    };

    use super::*;

    #[test]
    fn simple() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_id_column,
                 ..
             }| {
                let adelete = AbstractDelete {
                    table: concerts_table,
                    selection: AbstractSelect {
                        table: concerts_table,
                        selection: Selection::Seq(vec![ColumnSelection::new(
                            "id".to_string(),
                            SelectionElement::Physical(concerts_id_column),
                        )]),
                        predicate: Predicate::True,
                        order_by: None,
                        offset: None,
                        limit: None,
                    },
                    predicate: Predicate::True,
                };

                let delete = Postgres {}.to_delete(&adelete);
                let mut expr = ExpressionContext::default();
                let binding = delete.binding(&mut expr);
                assert_binding!(
                    binding,
                    r#"WITH "concerts" AS (DELETE FROM "concerts" WHERE true RETURNING *) select "concerts"."id" from "concerts""#
                );
            },
        );
    }

    #[test]
    fn nested_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_id_column,
                 concerts_venue_id_column,
                 venues_id_column,
                 venues_name_column,
                 venues_table,
                 ..
             }| {
                let predicate = AbstractPredicate::Eq(
                    ColumnPath::Physical(vec![
                        ColumnPathLink {
                            self_column: (concerts_venue_id_column, concerts_table),
                            linked_column: Some((venues_id_column, venues_table)),
                        },
                        ColumnPathLink {
                            self_column: (venues_name_column, venues_table),
                            linked_column: None,
                        },
                    ])
                    .into(),
                    ColumnPath::Literal(MaybeOwned::Owned(Box::new("v1".to_string()))).into(),
                );

                let adelete = AbstractDelete {
                    table: concerts_table,
                    selection: AbstractSelect {
                        table: concerts_table,
                        selection: Selection::Seq(vec![ColumnSelection::new(
                            "id".to_string(),
                            SelectionElement::Physical(concerts_id_column),
                        )]),
                        predicate: Predicate::True,
                        order_by: None,
                        offset: None,
                        limit: None,
                    },
                    predicate,
                };

                let delete = Postgres {}.to_delete(&adelete);
                let mut expr = ExpressionContext::default();
                let binding = delete.binding(&mut expr);

                // current:  DELETE FROM "concerts" WHERE "venues"."name" = $1 RETURNING *
                // expected: DELETE FROM "concerts" WHERE "concerts"."venue_id" in (select "venues"."id" from "venues" where "venues"."name" = $1) RETURNING *
                assert_binding!(
                    binding,
                    r#"WITH "concerts" AS (DELETE FROM "concert" WHERE "concert"."venue_id" in (select "venues"."id" from "venues" where "venues"."name" = $1) RETURNING *) select "concerts"."id" from "concerts""#,
                    "v1".to_string()
                );
            },
        );
    }
}
