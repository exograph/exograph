// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Transform an abstract insert into a concrete insert for Postgres.
//!
//! This allows us to execute GraphQL mutations like this:
//!
//! ```graphql
//! mutation {
//!   createVenue(data: {name: "v1", published: true, latitude: 1.2, concerts: [
//!     {title: "c1", published: true, price: 1.2},
//!     {title: "c2", published: false, price: 2.4}
//!   ]}) {
//!     id
//!   }
//! }
//! ```

use std::collections::{HashMap, HashSet};

use maybe_owned::MaybeOwned;
use tracing::instrument;

use crate::{
    asql::{
        insert::ColumnValuePair,
        insert::{AbstractInsert, NestedInsertion},
    },
    sql::{
        column::Column,
        cte::{CteExpression, WithQuery},
        predicate::ConcretePredicate,
        select::Select,
        sql_operation::SQLOperation,
        table::Table,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::transformer::{InsertTransformer, SelectTransformer},
    ColumnId, Database, Limit, Offset,
};

use super::Postgres;

impl InsertTransformer for Postgres {
    #[instrument(
        name = "InsertTransformer::to_transaction_script for Postgres"
        skip(self)
        )]
    fn to_transaction_script<'a>(
        &self,
        abstract_insert: &'a AbstractInsert,
        database: &'a Database,
    ) -> TransactionScript<'a> {
        let AbstractInsert {
            table_id,
            rows,
            selection,
        } = abstract_insert;

        let (self_rows, mut nested_rows): (Vec<_>, Vec<_>) = rows
            .iter()
            .map(|row| row.partition_self_and_nested())
            .unzip();

        // Align the columns and values for the top-level table. This way, if there are multiple rows,
        // we can insert them in a single statement.
        let (self_column_ids, self_column_values_seq) = align(self_rows);

        // Insert statements for the top-level table.
        // For a single row insertion, this will look like:
        // ```sql
        // INSERT INTO "venues" ("name", "published", "latitude") VALUES ($1, $2, $3) RETURNING *
        // ```
        // For multiple row insertion, this will look like:
        // ```sql
        // INSERT INTO "venues" ("name", "published", "latitude") VALUES ($1, $2, $3), ($4, $5, $6), ($7, $8, $9) RETURNING *
        // ```
        //
        // TODO: We need a different way to create TransactionScript for multiple rows. The current
        // approach is correct, but not conducive to using prepared statements, since the number of
        // parameters varies depending on columns and rows.
        // Specifically, we need to create a new `insert` for each row, get the id from each
        // inserted row, and then use those ids in the predicate while forming the final `select`
        // (`select ... from <table> where <table>.id in (<collected ids>)`)
        let table = database.get_table(*table_id);

        let self_columns = self_column_ids
            .iter()
            .map(|column_id| column_id.get_column(database))
            .collect::<Vec<_>>();

        let root_insert = SQLOperation::Insert(table.insert(
            self_columns,
            self_column_values_seq,
            vec![Column::Star(None).into()],
        ));

        let nested_rows = {
            let non_empty_nested_count = nested_rows
                .iter()
                .filter(|nested| !nested.is_empty())
                .count();

            if non_empty_nested_count == 1 {
                nested_rows.remove(0)
            } else if non_empty_nested_count == 0 {
                vec![]
            } else {
                // Currently, we don't support multiple top-level insertions with multiple nested insertions such as:
                // ```
                // mutation {
                //     createVenues(data: [
                //       {
                //         name: "v1",
                //         published: true,
                //         latitude: 1.2,
                //         concerts: [
                //           {title: "c1", published: true, price: 1.2},
                //           {title: "c2", published: false, price: 2.4}
                //         ]
                //       },
                //       {
                //         name: "v2",
                //         published: false,
                //         latitude: 2.4,
                //         concerts: [
                //           {title: "c4", published: true, price: 10.2},
                //           {title: "c4", published: false, price: 20.4}
                //         ]
                //       },
                //     ]) {
                //       id
                //     }
                //   }
                // ```
                // When we want to support this, probably we will have to process each nested insertion
                // separately, and put it in a transaction with the root insertion. We will also need
                // to collect the ids of the inserted rows from the root insertion, and use those ids
                // to form the final `select` statement (see how we do it in update_transformer.rs)
                panic!("Multiple top-level insertions with nested insertions not supported")
            }
        };

        let select = self.to_select(selection, database);

        let mut transaction_script = TransactionScript::default();

        if !nested_rows.is_empty() {
            // Insert statements for the nested tables such as:
            // ```sql
            // WITH
            //     "venues" AS (
            //       INSERT INTO "venues" ("name", "published", "latitude") VALUES ($1, $2, $3) RETURNING *
            //     ),
            //     "concerts" AS (
            //       INSERT INTO "concerts" ("price", "published", "title", "venue_id") VALUES
            //         ($4, $5, $6, (SELECT "venues"."id" FROM "venues")),
            //         ($7, $8, $9, (SELECT "venues"."id" FROM "venues")) RETURNING *
            //     )
            // SELECT json_build_object('id', "venues"."id")::text FROM "venues"
            // ```
            let nested_ctes = nested_rows.into_iter().map(
                |NestedInsertion {
                     relation_column_id,
                     parent_table,
                     insertions,
                 }| {
                    let self_insertion_elems = insertions
                        .iter()
                        .map(|insertion| insertion.partition_self_and_nested().0)
                        .collect();
                    let (mut column_ids, mut column_values_seq) = align(self_insertion_elems);
                    column_ids.push(*relation_column_id);

                    // To form the `(SELECT "venues"."id" FROM "venues")` part
                    let parent_pk_physical_column = database
                        .get_pk_column(*table_id)
                        .expect("Could not find primary key");
                    let parent_index: Option<u32> = None;
                    column_values_seq.iter_mut().for_each(|value| {
                        let parent_reference = Column::SubSelect(Box::new(Select {
                            table: Table::Physical(*parent_table),
                            columns: vec![Column::Physical(parent_pk_physical_column)],
                            predicate: ConcretePredicate::True,
                            order_by: None,
                            offset: parent_index.map(|index| Offset(index as i64)),
                            limit: parent_index.map(|_| Limit(1)),
                            group_by: None,
                            top_level_selection: false,
                        }));

                        value.push(parent_reference.into())
                    });

                    // Nested insert CTE. In the example above the `"concerts" AS ( ... )` part
                    let relation_table = database.get_table(relation_column_id.table_id);
                    let columns = column_ids
                        .iter()
                        .map(|column_id| column_id.get_column(database))
                        .collect::<Vec<_>>();
                    CteExpression {
                        name: relation_table.name.clone(),
                        operation: SQLOperation::Insert(relation_table.insert(
                            columns,
                            column_values_seq,
                            vec![Column::Star(None).into()],
                        )),
                    }
                },
            );

            // Root insert CTE. In the above example, the ` "venues" AS ( ... )` part
            let mut ctes = vec![CteExpression {
                name: table.name.clone(),
                operation: root_insert,
            }];
            ctes.extend(nested_ctes);

            transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
                SQLOperation::WithQuery(WithQuery {
                    expressions: ctes,
                    select,
                }),
            )));
        } else {
            // A WITH query that uses the `root_insert` as a CTE and then selects from it.
            // `WITH "venues" AS <the root insert above> <the select above>`. For example:
            //
            // ```sql
            // WITH "venues" AS (
            //   INSERT INTO "venues" ("latitude", "name", "published") VALUES ($1, $2, $3), ($4, $5, $6) RETURNING *
            // ) SELECT COALESCE(json_agg(json_build_object('id', "venues"."id")), '[]'::json)::text FROM "venues"
            // ```
            transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
                SQLOperation::WithQuery(WithQuery {
                    expressions: vec![CteExpression {
                        name: table.name.clone(),
                        operation: root_insert,
                    }],
                    select,
                }),
            )));
        }

        transaction_script
    }
}

/// Align multiple SingleInsertion's to account for misaligned and missing columns.
/// For example, if the input is {data: [{a: 1, b: 2}, {a: 3, c: 4}]}, we will have the 'a' key in both
/// but only 'b' or 'c' keys in others. So we need align columns that can be supplied to an insert statement
/// (a, b, c), [(1, 2, null), (3, null, 4)]
pub fn align<'a>(
    unaligned: Vec<Vec<&'a ColumnValuePair>>,
) -> (Vec<ColumnId>, Vec<Vec<MaybeOwned<'a, Column>>>) {
    let mut all_keys = HashSet::new();
    for row in unaligned.iter() {
        for insertion_value in row.iter() {
            all_keys.insert(insertion_value.column);
        }
    }

    // We are forming a table
    // a | b    | c
    // 1 | 2    | null
    // 3 | null | 4

    // To make insertion efficient, we create a map of key -> column in the table, so in the above example
    // we would have {a: 0, b: 1, c: 2}

    let key_map = all_keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect::<HashMap<_, _>>();

    let keys_count = all_keys.len();

    let mut aligned: Vec<Vec<MaybeOwned<'a, Column>>> = Vec::with_capacity(unaligned.len());

    for unaligned_row in unaligned.into_iter() {
        let mut aligned_row: Vec<MaybeOwned<'a, Column>> = Vec::with_capacity(keys_count);

        for _ in 0..keys_count {
            aligned_row.push(Column::Null.into());
        }

        for ColumnValuePair { column, value } in unaligned_row.into_iter() {
            let col_index = key_map[&column];
            aligned_row[col_index] = MaybeOwned::Borrowed(value);
        }

        aligned.push(aligned_row);
    }

    (all_keys.into_iter().collect(), aligned)
}
