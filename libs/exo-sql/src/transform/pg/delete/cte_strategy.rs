// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    sql::cte::{CteExpression, WithQuery},
    transform::{pg::precheck::add_precheck_queries, transformer::PredicateTransformer},
};

use crate::{
    sql::{
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::{
        pg::{selection_level::SelectionLevel, Postgres},
        transformer::SelectTransformer,
    },
    AbstractDelete, Column, Database,
};

use super::delete_strategy::DeleteStrategy;

pub(crate) struct CteStrategy {}

impl DeleteStrategy for CteStrategy {
    fn id(&self) -> &'static str {
        "CteStrategy"
    }

    fn suitable(&self, _abstract_delete: &AbstractDelete, _database: &Database) -> bool {
        true
    }

    fn update_transaction_script<'a>(
        &self,
        abstract_delete: AbstractDelete,
        database: &'a Database,
        transformer: &Postgres,
        transaction_script: &mut TransactionScript<'a>,
    ) {
        let AbstractDelete {
            table_id,
            predicate,
            selection,
            precheck_predicates,
        } = abstract_delete;

        add_precheck_queries(
            precheck_predicates,
            database,
            transformer,
            transaction_script,
        );

        let delete_query = to_delete(
            AbstractDelete {
                table_id,
                predicate,
                selection,
                precheck_predicates: vec![],
            },
            database,
            transformer,
        );

        let _ = transaction_script.add_step(TransactionStep::Concrete(
            ConcreteTransactionStep::new(SQLOperation::WithQuery(delete_query)),
        ));
    }
}

fn to_delete<'a>(
    abstract_delete: AbstractDelete,
    database: &'a Database,
    transformer: &Postgres,
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
    let predicate = transformer.to_predicate(
        &abstract_delete.predicate,
        &SelectionLevel::TopLevel,
        false,
        database,
    );

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
    let select = transformer.to_select(abstract_delete.selection, database);

    // A WITH query that uses the `root_delete` as a CTE and then selects from it.
    // `WITH "concerts" AS <the delete above> <the select above>`. For example:
    //
    // ```sql
    // WITH "concerts" AS (
    //    DELETE FROM WHERE <the predicate above> RETURNING *
    // ) SELECT COALESCE(...)::text AS "concerts"
    // ```
    let table_name = &database.get_table(abstract_delete.table_id).name;
    WithQuery {
        expressions: vec![CteExpression::new_auto_name(table_name, root_delete)],
        select,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        asql::selection::{AliasedSelectionElement, Selection, SelectionElement},
        sql::{predicate::Predicate, ExpressionBuilder, SQLParamContainer},
        transform::{pg::Postgres, test_util::TestSetup},
        AbstractPredicate, AbstractSelect, ColumnPath, PhysicalColumnPath,
    };

    use multiplatform_test::multiplatform_test;

    use super::*;

    #[multiplatform_test]
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
                    precheck_predicates: vec![],
                };

                let delete = to_delete(adelete, &database, &Postgres {});
                assert_binding!(
                    delete.to_sql(&database),
                    r#"WITH "concerts" AS (DELETE FROM "concerts" RETURNING *) SELECT "concerts"."id" FROM "concerts""#
                );
            },
        );
    }

    #[multiplatform_test]
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
                    ColumnPath::Physical(PhysicalColumnPath::leaf(concerts_name_column)),
                    ColumnPath::Param(SQLParamContainer::string("v1".to_string())),
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
                    precheck_predicates: vec![],
                };

                let delete = to_delete(adelete, &database, &Postgres {});

                assert_binding!(
                    delete.to_sql(&database),
                    r#"WITH "concerts" AS (DELETE FROM "concerts" WHERE "concerts"."name" = $1 RETURNING *) SELECT "concerts"."id" FROM "concerts""#,
                    "v1".to_string()
                );
            },
        );
    }

    #[multiplatform_test]
    fn nested_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_table,
                 concerts_id_column,
                 concerts_venue_id_column,
                 venues_name_column,
                 ..
             }| {
                let predicate = AbstractPredicate::Eq(
                    ColumnPath::Physical(PhysicalColumnPath::from_columns(
                        vec![concerts_venue_id_column, venues_name_column],
                        &database,
                    )),
                    ColumnPath::Param(SQLParamContainer::string("v1".to_string())),
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
                    precheck_predicates: vec![],
                };

                let delete = to_delete(adelete, &database, &Postgres {});

                assert_binding!(
                    delete.to_sql(&database),
                    r#"WITH "concerts" AS (DELETE FROM "concerts" WHERE "concerts"."venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE "venues"."name" = $1) RETURNING *) SELECT "concerts"."id" FROM "concerts""#,
                    "v1".to_string()
                );
            },
        );
    }
}
