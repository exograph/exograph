use std::collections::{HashMap, HashSet};

use maybe_owned::MaybeOwned;

use crate::sql::{
    column::{Column, PhysicalColumn},
    predicate::Predicate,
    transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    Cte, Limit, Offset, PhysicalTable, SQLOperation, Select, TableQuery,
};

use super::{
    select::{AbstractSelect, SelectionLevel},
    selection::NestedElementRelation,
};

#[derive(Debug)]
pub struct InsertionColumnValuePair<'a> {
    column: &'a PhysicalColumn,
    value: MaybeOwned<'a, Column<'a>>,
}

impl<'a> InsertionColumnValuePair<'a> {
    pub fn new(column: &'a PhysicalColumn, value: MaybeOwned<'a, Column<'a>>) -> Self {
        Self { column, value }
    }
}

#[derive(Debug)]
pub struct NestedInsertion<'a> {
    pub relation: NestedElementRelation<'a>,
    pub self_table: &'a PhysicalTable,
    pub parent_table: &'a PhysicalTable,
    pub insertions: Vec<InsertionRow<'a>>,
}

/// Logical element (of a logical row) to be inserted.
/// Each element may be a column-value pair, or a nested insertion.
/// For example, inserting a venue may specify a column-value pair for the venue's name,
/// or an associated concert (whose venue_id should be set to the inserted venue's id).
#[derive(Debug)]
pub enum InsertionElement<'a> {
    SelfInsert(InsertionColumnValuePair<'a>),
    NestedInsert(NestedInsertion<'a>),
}

#[derive(Debug)]
pub struct InsertionRow<'a> {
    pub elems: Vec<InsertionElement<'a>>,
}

#[derive(Debug)]
pub struct AbstractInsert<'a> {
    pub table: &'a PhysicalTable,
    pub rows: Vec<InsertionRow<'a>>,
    pub selection: AbstractSelect<'a>,
}

impl<'a> InsertionRow<'a> {
    fn partition_self_and_nested(
        self,
    ) -> (Vec<InsertionColumnValuePair<'a>>, Vec<NestedInsertion<'a>>) {
        let mut self_elems = Vec::new();
        let mut nested_elems = Vec::new();
        for elem in self.elems {
            match elem {
                InsertionElement::SelfInsert(pair) => self_elems.push(pair),
                InsertionElement::NestedInsert(nested) => nested_elems.push(nested),
            }
        }
        (self_elems, nested_elems)
    }
}

impl<'a> AbstractInsert<'a> {
    pub fn to_sql(self) -> TransactionScript<'a> {
        let AbstractInsert {
            table,
            rows,
            selection,
        } = self;

        let select = selection.to_sql(None, SelectionLevel::TopLevel);

        let (self_elems, mut nested_elems): (Vec<_>, Vec<_>) = rows
            .into_iter()
            .map(|row| row.partition_self_and_nested())
            .unzip();

        let (column_names, column_values_seq) = align(self_elems);

        let root_update = SQLOperation::Insert(table.insert(
            column_names,
            column_values_seq,
            vec![Column::Star.into()],
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
                        .into_iter()
                        .map(|insertion| insertion.partition_self_and_nested().0)
                        .collect();
                    let (mut column_names, mut column_values_seq) = align(self_insertion_elems);
                    column_names.push(relation.column);

                    let parent_pk_physical_column = self
                        .table
                        .get_pk_physical_column()
                        .expect("Could not find primary key");
                    let parent_index: Option<u32> = None;
                    column_values_seq.iter_mut().for_each(|value| {
                        let parent_reference = Column::SelectionTableWrapper(Box::new(Select {
                            underlying: TableQuery::Physical(parent_table),
                            columns: vec![Column::Physical(parent_pk_physical_column).into()],
                            predicate: Predicate::True.into(),
                            order_by: None,
                            offset: parent_index.map(|index| Offset(index as i64)),
                            limit: parent_index.map(|_| Limit(1)),
                            top_level_selection: false,
                        }));

                        value.push(parent_reference.into())
                    });
                    (
                        self_table.name.clone(),
                        SQLOperation::Insert(self_table.insert(
                            column_names,
                            column_values_seq,
                            vec![Column::Star.into()],
                        )),
                    )
                },
            );

            let mut ctes = vec![(table.name.clone(), root_update)];
            ctes.extend(nested_ctes);

            transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
                SQLOperation::Cte(Cte { ctes, select }),
            )));
        } else {
            transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
                SQLOperation::Cte(Cte {
                    ctes: vec![(table.name.clone(), root_update)],
                    select,
                }),
            )));
        }

        transaction_script
    }

    // fn get_self_insertion_rows(
    //     row: Vec<InsertionElement<'a>>,
    // ) -> Vec<InsertionColumnValuePair<'a>> {
    //     row.into_iter()
    //         .filter_map(|elem| match elem {
    //             InsertionElement::SelfInsert(values) => Some(values),
    //             _ => None,
    //         })
    //         .collect()
    // }
}

/// Align multiple SingleInsertion's to account for misaligned and missing columns
/// For example, if the input is {data: [{a: 1, b: 2}, {a: 3, c: 4}]}, we will have the 'a' key in both
/// but only 'b' or 'c' keys in others. So we need align columns that can be supplied to an insert statement
/// (a, b, c), [(1, 2, null), (3, null, 4)]
fn align(
    unaligned: Vec<Vec<InsertionColumnValuePair>>,
) -> (Vec<&PhysicalColumn>, Vec<Vec<MaybeOwned<Column>>>) {
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

    let mut aligned = Vec::with_capacity(unaligned.len());

    // let mut values = Vec::with_capacity(unaligned.len());
    // let mut nested = vec![];

    for unaligned_row in unaligned.into_iter() {
        let mut aligned_row = Vec::with_capacity(keys_count);

        for _ in 0..keys_count {
            aligned_row.push(Column::Null.into());
        }

        for InsertionColumnValuePair { column, value } in unaligned_row.into_iter() {
            let col_index = key_map[&column];
            aligned_row[col_index] = value;
        }

        aligned.push(aligned_row);
    }

    (all_keys.into_iter().collect(), aligned)

    // for mut item in unaligned.into_iter() {
    //     let mut row = Vec::with_capacity(keys_count);
    //     for key in &all_keys {
    //         for insertion_value in item.iter_mut() {
    //             let value = insertion_value
    //                 .self_row
    //                 .remove(key)
    //                 .map(|v| v.into())
    //                 .unwrap_or_else(|| Column::Null.into());
    //             row.push(value);
    //         }
    //     }

    //     values.push(row);
    //     nested.extend(item.nested_rows);
    // }

    // InsertionInfo {
    //     table,
    //     columns: all_keys.into_iter().collect(),
    //     values,
    //     nested,
    // }
}
