// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{HashMap, HashSet};

use maybe_owned::MaybeOwned;

use crate::{asql::insert::ColumnValuePair, sql::column::Column, ColumnId};

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
