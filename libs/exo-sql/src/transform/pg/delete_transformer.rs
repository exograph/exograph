// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use tracing::instrument;

use crate::{
    asql::delete::AbstractDelete,
    sql::{
        column::Column,
        cte::{CteExpression, WithQuery},
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::transformer::{DeleteTransformer, PredicateTransformer, SelectTransformer},
    Database,
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
        database: &'a Database,
    ) -> TransactionScript<'a> {
        let delete = self.to_delete(abstract_delete, database);
        let mut transaction_script = TransactionScript::default();
        transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
            SQLOperation::WithQuery(delete),
        )));
        transaction_script
    }

    #[instrument(name = "DeleteTransformer::to_delete for Postgres", skip(self))]
    fn to_delete<'a>(
        &self,
        abstract_delete: &'a AbstractDelete,
        database: &'a Database,
    ) -> WithQuery<'a> {
        // The concrete predicate created here will be a direct predicate if the abstract predicate
        // refers to the columns of the table being deleted. For example, to delete concerts based
        // on its title, the predicate will be:
        // ```sql
        // WHERE "concerts"."title" = $1
        // ```
        // However, if the abstract predicate refers to columns of a related table, we need to
        // generate a subselect. For example, to delete concerts based on the venue's name, the
        // predicate will be:
        // ```sql
        // DELETE FROM "concerts" WHERE "concerts"."id" IN (
        //    SELECT "concerts"."id" FROM "concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id" where "venues"."name" = $1
        // )
        // ```
        // We need a subselect because the target of a DELETE statement must be a physical table,
        // not a join.
        let predicate = self.to_predicate(&abstract_delete.predicate, false, database);

        // The root delete operation returning all columns of the table being deleted. This will be
        // later used as a CTE.
        // ```sql
        // DELETE FROM "concerts" WHERE <the predicate above> RETURNING *
        // ```
        let root_delete = SQLOperation::Delete(
            database
                .get_table(abstract_delete.table_id)
                .delete(predicate, vec![Column::Star(None)]),
        );

        // The select (often a json aggregation)
        let select = self.to_select(&abstract_delete.selection, database);

        // A WITH query that uses the `root_delete` as a CTE and then selects from it.
        // `WITH "concerts" AS <the delete above> <the select above>`. For example:
        //
        // ```sql
        // WITH "concerts" AS (
        //    DELETE FROM WHERE <the predicate above> RETURNING *
        // ) SELECT COALESCE(...)::text AS "concerts"
        // ```
        WithQuery {
            expressions: vec![CteExpression {
                name: database.get_table(abstract_delete.table_id).name.clone(),
                operation: root_delete,
            }],
            select,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        asql::{
            column_path::PhysicalColumnPathLink,
            selection::{AliasedSelectionElement, Selection, SelectionElement},
        },
        sql::{predicate::Predicate, ExpressionBuilder, SQLParamContainer},
        transform::{pg::Postgres, test_util::TestSetup},
        AbstractPredicate, AbstractSelect, ColumnPath,
    };

    use super::*;

    #[test]
    fn delete_all() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_table,
                 concerts_id_column,
                 ..
             }| {
                let adelete = AbstractDelete {
                    table_id: concerts_table,
                    selection: AbstractSelect {
                        table_id: concerts_table,
                        selection: Selection::Seq(vec![AliasedSelectionElement::new(
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

                let delete = Postgres {}.to_delete(&adelete, &database);
                assert_binding!(
                    delete.to_sql(&database),
                    r#"WITH "concerts" AS (DELETE FROM "concerts" RETURNING *) SELECT "concerts"."id" FROM "concerts""#
                );
            },
        );
    }

    #[test]
    fn non_nested_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_table,
                 concerts_id_column,
                 concerts_name_column,
                 ..
             }| {
                let predicate = AbstractPredicate::Eq(
                    ColumnPath::Physical(vec![PhysicalColumnPathLink::Leaf(concerts_name_column)]),
                    ColumnPath::Param(SQLParamContainer::new("v1".to_string())),
                );

                let adelete = AbstractDelete {
                    table_id: concerts_table,
                    selection: AbstractSelect {
                        table_id: concerts_table,
                        selection: Selection::Seq(vec![AliasedSelectionElement::new(
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

                let delete = Postgres {}.to_delete(&adelete, &database);

                assert_binding!(
                    delete.to_sql(&database),
                    r#"WITH "concerts" AS (DELETE FROM "concerts" WHERE "concerts"."name" = $1 RETURNING *) SELECT "concerts"."id" FROM "concerts""#,
                    "v1".to_string()
                );
            },
        );
    }

    #[test]
    fn nested_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_table,
                 concerts_id_column,
                 concerts_venue_id_column,
                 venues_id_column,
                 venues_name_column,
                 ..
             }| {
                let predicate = AbstractPredicate::Eq(
                    ColumnPath::Physical(vec![
                        PhysicalColumnPathLink::relation(
                            concerts_venue_id_column,
                            venues_id_column,
                        ),
                        PhysicalColumnPathLink::Leaf(venues_name_column),
                    ]),
                    ColumnPath::Param(SQLParamContainer::new("v1".to_string())),
                );

                let adelete = AbstractDelete {
                    table_id: concerts_table,
                    selection: AbstractSelect {
                        table_id: concerts_table,
                        selection: Selection::Seq(vec![AliasedSelectionElement::new(
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

                let delete = Postgres {}.to_delete(&adelete, &database);

                assert_binding!(
                    delete.to_sql(&database),
                    r#"WITH "concerts" AS (DELETE FROM "concerts" WHERE "concerts"."venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE "venues"."name" = $1) RETURNING *) SELECT "concerts"."id" FROM "concerts""#,
                    "v1".to_string()
                );
            },
        );
    }
}
