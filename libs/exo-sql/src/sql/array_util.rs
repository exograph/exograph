// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{HashMap, hash_map::Entry};

use crate::{
    database_error::DatabaseError,
    sql::physical_column_type::{ArrayColumnType, JsonColumnType, PhysicalColumnType},
};

use super::SQLParamContainer;
use postgres_array::{Array, Dimension};
use tokio_postgres::types::Type;

pub enum ArrayEntry<'a, T> {
    Single(&'a T),
    List(&'a Vec<T>),
}

type OptionalSQLParam = Option<SQLParamContainer>;

/// Convert a Rust array into an SQLParam.
///
/// Postgres's multi-dimensional arrays are represented as a single array of
/// elements in a row-major order. This function processes the elements whose content
/// may be a single element (ArrayEntry::Single) or a list of elements (ArrayEntry::List).
///# Arguments
/// * `elems` - The array to convert.
/// * `destination_type` - The type of the array of the primitive element in the database.
/// * `array_entry` - A function to convert an element of an array to an ArrayEntry (ArrayEntry::Single or ArrayEntry::List).
/// * `to_sql_param` - A function to convert an element of an array to an SQLParam.
pub fn to_sql_param<T>(
    elems: &[T],
    destination_type: &dyn PhysicalColumnType,
    array_entry: fn(&T) -> ArrayEntry<T>,
    to_sql_param: &impl Fn(&T) -> Result<OptionalSQLParam, DatabaseError>,
) -> Result<Option<SQLParamContainer>, DatabaseError> {
    let element_pg_type =
        if let Some(array_type) = destination_type.as_any().downcast_ref::<ArrayColumnType>() {
            array_type.typ.get_pg_type()
        } else if destination_type.as_any().is::<JsonColumnType>() {
            Type::JSONB
        } else {
            return Err(DatabaseError::Validation(
                "Destination type is not an array".to_string(),
            ));
        };
    to_sql_array(elems, element_pg_type, array_entry, to_sql_param).map(|array| {
        Some(SQLParamContainer::new(
            array,
            destination_type.get_pg_type(),
        ))
    })
}

// Separate function to enable testing
fn to_sql_array<T>(
    elems: &[T],
    element_pg_type: Type,
    array_entry: fn(&T) -> ArrayEntry<T>,
    to_sql_param: &impl Fn(&T) -> Result<OptionalSQLParam, DatabaseError>,
) -> Result<Array<SQLParamContainer>, DatabaseError> {
    let mut result = (Vec::new(), HashMap::new());
    process_array(
        elems,
        &element_pg_type,
        &mut result,
        0,
        array_entry,
        to_sql_param,
    )?;

    let mut dimension_lens = result.1.iter().collect::<Vec<_>>();
    dimension_lens.sort_by_key(|(key, _)| **key);
    let dimensions = dimension_lens
        .into_iter()
        .map(|(_, v)| Dimension {
            len: *v,
            lower_bound: 0,
        })
        .collect::<Vec<_>>();

    Ok(Array::from_parts(result.0, dimensions))
}

/// Process a (possibly nested) array of values to extract information to use it as Postgres parameter.
///
/// The output is the `result` param that has flattened all the elements and a set of dimension->value mapping.
/// See the tests module for examples.
fn process_array<T>(
    elems: &[T],
    element_pg_type: &Type,
    result: &mut (Vec<SQLParamContainer>, HashMap<usize, i32>),
    depth: usize,
    array_entry: fn(&T) -> ArrayEntry<T>,
    to_sql_param: &impl Fn(&T) -> Result<OptionalSQLParam, DatabaseError>,
) -> Result<(), DatabaseError> {
    let mut len = 0;

    for elem in elems {
        len += 1;
        match array_entry(elem) {
            ArrayEntry::Single(elem) => {
                let value = to_sql_param(elem)?;
                result
                    .0
                    .push(SQLParamContainer::new(value, element_pg_type.clone()));
            }
            ArrayEntry::List(elems) => {
                process_array(
                    elems,
                    element_pg_type,
                    result,
                    depth + 1,
                    array_entry,
                    to_sql_param,
                )?;
            }
        }
    }

    // Update the dimension if this is the first time we are at this depth
    // If this is a repeated visit at a depth, check if the length is the same
    // (we do not support entries in the array of different lengths)
    match result.1.entry(depth) {
        Entry::Vacant(entry) => {
            entry.insert(len);
        }
        Entry::Occupied(entry) => {
            if *entry.get() != len {
                return Err(DatabaseError::Validation(format!(
                    "Array dimensions do not match in dimension {}. Expected {}, got {}",
                    depth,
                    *entry.get(),
                    len
                )));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use multiplatform_test::multiplatform_test;

    // Emulate just sufficient ConstValue in async-graphql
    enum Element {
        Single(i32),
        List(Vec<Element>),
    }

    fn i32_to_sql_param(i: &i32) -> Result<OptionalSQLParam, DatabaseError> {
        Ok(Some(SQLParamContainer::i32(*i) as SQLParamContainer))
    }

    fn element_to_sql_param(
        entry: &Element,
        element_pg_type: &Type,
    ) -> Result<OptionalSQLParam, DatabaseError> {
        match entry {
            Element::Single(i) => Ok(Some(
                SQLParamContainer::new(*i, element_pg_type.clone()) as SQLParamContainer
            )),
            Element::List(entries) => {
                let mut result = Vec::new();
                for entry in entries {
                    result.push(element_to_sql_param(entry, element_pg_type)?);
                }
                Ok(Some(
                    SQLParamContainer::new(result, element_pg_type.clone()) as SQLParamContainer,
                ))
            }
        }
    }

    fn to_debug_string(array: &Array<SQLParamContainer>) -> Vec<String> {
        array.iter().map(|e| format!("{:?}", e.param())).collect()
    }

    #[multiplatform_test]
    fn single_dimensional() {
        let elems = vec![1, 2, 3];

        fn array_entry(elem: &i32) -> ArrayEntry<'_, i32> {
            ArrayEntry::Single(elem)
        }

        let array = to_sql_array(&elems, Type::INT4, array_entry, &i32_to_sql_param).unwrap();
        assert_eq!(
            array.dimensions(),
            [Dimension {
                len: 3,
                lower_bound: 0,
            }]
        );
        assert_eq!(
            to_debug_string(&array),
            [
                "SQLParamWithType { param: Some(SQLParamWithType { param: 1, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 2, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 3, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }"
            ]
        );
    }

    #[multiplatform_test]
    fn two_dimensional() {
        let elems = vec![
            Element::List(vec![
                Element::Single(1),
                Element::Single(2),
                Element::Single(3),
            ]),
            Element::List(vec![
                Element::Single(4),
                Element::Single(5),
                Element::Single(6),
            ]),
        ];

        fn array_entry(elem: &Element) -> ArrayEntry<'_, Element> {
            match elem {
                Element::List(elems) => ArrayEntry::List(elems),
                _ => ArrayEntry::Single(elem),
            }
        }

        fn to_sql_param(elem: &Element) -> Result<OptionalSQLParam, DatabaseError> {
            element_to_sql_param(elem, &Type::INT4)
        }
        let array = to_sql_array(&elems, Type::INT4, array_entry, &to_sql_param).unwrap();
        assert_eq!(
            array.dimensions(),
            [
                Dimension {
                    len: 2,
                    lower_bound: 0,
                },
                Dimension {
                    len: 3,
                    lower_bound: 0,
                }
            ]
        );
        assert_eq!(
            to_debug_string(&array),
            vec![
                "SQLParamWithType { param: Some(SQLParamWithType { param: 1, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 2, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 3, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 4, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 5, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 6, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }"
            ]
        );
    }

    #[multiplatform_test]
    fn three_dimensional() {
        let elems = vec![
            Element::List(vec![Element::List(vec![
                Element::Single(1),
                Element::Single(2),
                Element::Single(3),
            ])]),
            Element::List(vec![Element::List(vec![
                Element::Single(4),
                Element::Single(5),
                Element::Single(6),
            ])]),
        ];

        fn array_entry(elem: &Element) -> ArrayEntry<'_, Element> {
            match elem {
                Element::List(elems) => ArrayEntry::List(elems),
                _ => ArrayEntry::Single(elem),
            }
        }

        fn to_sql_param(elem: &Element) -> Result<OptionalSQLParam, DatabaseError> {
            element_to_sql_param(elem, &Type::INT4)
        }
        let array = to_sql_array(&elems, Type::INT4, array_entry, &to_sql_param).unwrap();
        assert_eq!(
            array.dimensions(),
            [
                Dimension {
                    len: 2,
                    lower_bound: 0,
                },
                Dimension {
                    len: 1,
                    lower_bound: 0,
                },
                Dimension {
                    len: 3,
                    lower_bound: 0,
                }
            ]
        );
        assert_eq!(
            to_debug_string(&array),
            vec![
                "SQLParamWithType { param: Some(SQLParamWithType { param: 1, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 2, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 3, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 4, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 5, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }",
                "SQLParamWithType { param: Some(SQLParamWithType { param: 6, param_type: Int4, is_array: false, enum_type: None }), param_type: Int4, is_array: false, enum_type: None }"
            ]
        );
    }
}
