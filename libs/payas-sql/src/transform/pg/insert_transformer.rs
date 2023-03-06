use std::collections::{HashMap, HashSet};

use maybe_owned::MaybeOwned;
use tracing::instrument;

use crate::{
    asql::{
        insert::ColumnValuePair,
        insert::{AbstractInsert, NestedInsertion},
        select::SelectionLevel,
    },
    sql::{
        column::{Column, PhysicalColumn},
        cte::{Cte, CteExpression},
        predicate::ConcretePredicate,
        select::Select,
        sql_operation::SQLOperation,
        table::TableQuery,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::transformer::{InsertTransformer, SelectTransformer},
    Limit, Offset,
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
    ) -> TransactionScript<'a> {
        let AbstractInsert {
            table,
            rows,
            selection,
        } = abstract_insert;

        let select = self.to_select(selection, None, None, SelectionLevel::TopLevel);

        let (self_elems, mut nested_elems): (Vec<_>, Vec<_>) = rows
            .iter()
            .map(|row| row.partition_self_and_nested())
            .unzip();

        let (column_names, column_values_seq) = align(self_elems);

        let root_update = SQLOperation::Insert(table.insert(
            column_names,
            column_values_seq,
            vec![Column::Star(None).into()],
        ));

        // TODO: We need a different way to create TransactionScript for multiple rows
        // Specifically, we need to create a new `insert` for each row, get the id from each inserted row,
        // and then use those id in the predicate while forming the final `select` (`select ... from <table> where <table>.id in (<collected ids>)`)
        let nested_elems = {
            let non_empty_nested_count = nested_elems
                .iter()
                .filter(|nested| !nested.is_empty())
                .count();

            if non_empty_nested_count == 1 {
                nested_elems.remove(0)
            } else if non_empty_nested_count == 0 {
                vec![]
            } else {
                panic!("Multiple top-level insertions with nested insertions not supported")
            }
        };

        let mut transaction_script = TransactionScript::default();

        if !nested_elems.is_empty() {
            let nested_ctes = nested_elems.into_iter().map(
                |NestedInsertion {
                     relation,
                     self_table,
                     parent_table,
                     insertions,
                 }| {
                    let self_insertion_elems = insertions
                        .iter()
                        .map(|insertion| insertion.partition_self_and_nested().0)
                        .collect();
                    let (mut column_names, mut column_values_seq) = align(self_insertion_elems);
                    column_names.push(relation.column);

                    let parent_pk_physical_column = table
                        .get_pk_physical_column()
                        .expect("Could not find primary key");
                    let parent_index: Option<u32> = None;
                    column_values_seq.iter_mut().for_each(|value| {
                        let parent_reference = Column::SelectionTableWrapper(Box::new(Select {
                            underlying: TableQuery::Physical(parent_table),
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
                    CteExpression {
                        name: self_table.name.clone(),
                        operation: SQLOperation::Insert(self_table.insert(
                            column_names,
                            column_values_seq,
                            vec![Column::Star(None).into()],
                        )),
                    }
                },
            );

            let mut ctes = vec![CteExpression {
                name: table.name.clone(),
                operation: root_update,
            }];
            ctes.extend(nested_ctes);

            transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
                SQLOperation::Cte(Cte {
                    expressions: ctes,
                    select,
                }),
            )));
        } else {
            transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
                SQLOperation::Cte(Cte {
                    expressions: vec![CteExpression {
                        name: table.name.clone(),
                        operation: root_update,
                    }],
                    select,
                }),
            )));
        }

        transaction_script
    }
}

/// Align multiple SingleInsertion's to account for misaligned and missing columns
/// For example, if the input is {data: [{a: 1, b: 2}, {a: 3, c: 4}]}, we will have the 'a' key in both
/// but only 'b' or 'c' keys in others. So we need align columns that can be supplied to an insert statement
/// (a, b, c), [(1, 2, null), (3, null, 4)]
pub fn align<'a>(
    unaligned: Vec<Vec<&'a ColumnValuePair>>,
) -> (
    Vec<&'a PhysicalColumn>,
    Vec<Vec<MaybeOwned<'a, Column<'a>>>>,
) {
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

    let mut aligned: Vec<Vec<MaybeOwned<'a, Column<'a>>>> = Vec::with_capacity(unaligned.len());

    for unaligned_row in unaligned.into_iter() {
        let mut aligned_row: Vec<MaybeOwned<'a, Column<'a>>> = Vec::with_capacity(keys_count);

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
