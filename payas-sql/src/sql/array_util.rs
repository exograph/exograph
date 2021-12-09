use std::collections::{hash_map::Entry, HashMap};

use anyhow::{anyhow, Result};

use super::{column::PhysicalColumnType, SQLParam};
use postgres_array::{Array, Dimension};

pub enum ArrayEntry<'a, T> {
    Single(&'a T),
    List(&'a Vec<T>),
}

type OptionalSQLParam = Option<Box<dyn SQLParam>>;

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
    destination_type: &PhysicalColumnType,
    array_entry: fn(&T) -> ArrayEntry<T>,
    to_sql_param: fn(&T, &PhysicalColumnType) -> Result<OptionalSQLParam>,
) -> Result<Option<Box<dyn SQLParam>>> {
    to_sql_array(elems, destination_type, array_entry, to_sql_param)
        .map(|array| Some(Box::new(array) as Box<dyn SQLParam>))
}

// Separate function to enable testing
fn to_sql_array<T>(
    elems: &[T],
    destination_type: &PhysicalColumnType,
    array_entry: fn(&T) -> ArrayEntry<T>,
    to_sql_param: fn(&T, &PhysicalColumnType) -> Result<OptionalSQLParam>,
) -> Result<Array<Box<dyn SQLParam>>> {
    let mut result = (Vec::new(), HashMap::new());
    process_array(
        elems,
        destination_type,
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
    destination_type: &PhysicalColumnType,
    result: &mut (Vec<Box<dyn SQLParam>>, HashMap<usize, i32>),
    depth: usize,
    array_entry: fn(&T) -> ArrayEntry<T>,
    to_sql_param: fn(&T, &PhysicalColumnType) -> Result<OptionalSQLParam>,
) -> Result<()> {
    let mut len = 0;

    for elem in elems {
        len += 1;
        match array_entry(elem) {
            ArrayEntry::Single(elem) => {
                let value = to_sql_param(elem, destination_type)?;
                result.0.push(Box::new(value));
            }
            ArrayEntry::List(elems) => {
                process_array(
                    elems,
                    destination_type,
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
                return Err(anyhow!(
                    "Array dimensions do not match in dimention {}. Expected {}, got {}",
                    depth,
                    *entry.get(),
                    len
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Emulate just sufficient ConstValue in async-graphql
    enum Element {
        Single(i32),
        List(Vec<Element>),
    }

    fn i32_to_sql_param(i: &i32, _: &PhysicalColumnType) -> Result<OptionalSQLParam> {
        Ok(Some(Box::new(*i) as Box<dyn SQLParam>))
    }

    fn element_to_sql_param(entry: &Element, typ: &PhysicalColumnType) -> Result<OptionalSQLParam> {
        match entry {
            Element::Single(i) => Ok(Some(Box::new(*i) as Box<dyn SQLParam>)),
            Element::List(entries) => {
                let mut result = Vec::new();
                for entry in entries {
                    result.push(element_to_sql_param(entry, typ)?);
                }
                Ok(Some(Box::new(result) as Box<dyn SQLParam>))
            }
        }
    }

    fn to_debug_string(array: &Array<Box<dyn SQLParam>>) -> Vec<String> {
        array.iter().map(|e| format!("{:?}", e)).collect()
    }

    #[test]
    fn single_dimensional() {
        let elems = vec![1, 2, 3];
        let destination_type = PhysicalColumnType::Int {
            bits: crate::sql::column::IntBits::_32,
        };
        fn array_entry(elem: &i32) -> ArrayEntry<i32> {
            ArrayEntry::Single(elem)
        }

        let array = to_sql_array(&elems, &destination_type, array_entry, i32_to_sql_param).unwrap();
        assert_eq!(
            array.dimensions(),
            [Dimension {
                len: 3,
                lower_bound: 0,
            }]
        );
        assert_eq!(
            to_debug_string(&array),
            vec!["Some(1)", "Some(2)", "Some(3)",]
        );
    }

    #[test]
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
        let destination_type = PhysicalColumnType::Int {
            bits: crate::sql::column::IntBits::_32,
        };

        fn array_entry(elem: &Element) -> ArrayEntry<Element> {
            match elem {
                Element::List(elems) => ArrayEntry::List(elems),
                _ => ArrayEntry::Single(elem),
            }
        }

        let array =
            to_sql_array(&elems, &destination_type, array_entry, element_to_sql_param).unwrap();
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
            vec!["Some(1)", "Some(2)", "Some(3)", "Some(4)", "Some(5)", "Some(6)",]
        );
    }

    #[test]
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
        let destination_type = PhysicalColumnType::Int {
            bits: crate::sql::column::IntBits::_32,
        };

        fn array_entry(elem: &Element) -> ArrayEntry<Element> {
            match elem {
                Element::List(elems) => ArrayEntry::List(elems),
                _ => ArrayEntry::Single(elem),
            }
        }

        let array =
            to_sql_array(&elems, &destination_type, array_entry, element_to_sql_param).unwrap();
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
            vec!["Some(1)", "Some(2)", "Some(3)", "Some(4)", "Some(5)", "Some(6)",]
        );
    }
}
