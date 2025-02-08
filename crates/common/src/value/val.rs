// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{collections::HashMap, fmt::Display};

use async_graphql_value::ConstValue;
use serde::de::Error;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ValNumber {
    I32(i32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
}

impl ValNumber {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ValNumber::F32(n) => Some(*n as f64),
            ValNumber::F64(n) => Some(*n),
            ValNumber::I32(n) => Some(*n as f64),
            ValNumber::I64(n) => Some(*n as f64),
            ValNumber::U64(n) => Some(*n as f64),
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ValNumber::I32(n) => Some(*n as i64),
            ValNumber::I64(n) => Some(*n),
            ValNumber::U64(_) => None,
            ValNumber::F32(_) => None,
            ValNumber::F64(_) => None,
        }
    }
}

impl Display for ValNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValNumber::I32(n) => write!(f, "{n}"),
            ValNumber::I64(n) => write!(f, "{n}"),
            ValNumber::U64(n) => write!(f, "{n}"),
            ValNumber::F32(n) => write!(f, "{n}"),
            ValNumber::F64(n) => write!(f, "{n}"),
        }
    }
}

impl TryFrom<ValNumber> for serde_json::Number {
    type Error = ();

    fn try_from(value: ValNumber) -> Result<Self, Self::Error> {
        match value {
            ValNumber::I32(n) => Ok(serde_json::Number::from(n)),
            ValNumber::I64(n) => Ok(serde_json::Number::from(n)),
            ValNumber::U64(n) => Ok(serde_json::Number::from(n)),
            ValNumber::F32(n) => serde_json::Number::from_f64(n as f64).ok_or(()),
            ValNumber::F64(n) => serde_json::Number::from_f64(n).ok_or(()),
        }
    }
}

impl TryFrom<serde_json::Number> for ValNumber {
    type Error = ();

    fn try_from(value: serde_json::Number) -> Result<Self, Self::Error> {
        if value.is_i64() {
            Ok(ValNumber::I64(value.as_i64().unwrap()))
        } else if value.is_u64() {
            Ok(ValNumber::U64(value.as_u64().unwrap()))
        } else if value.is_f64() {
            Ok(ValNumber::F64(value.as_f64().unwrap()))
        } else {
            Err(())
        }
    }
}

impl From<i32> for ValNumber {
    fn from(value: i32) -> Self {
        ValNumber::I32(value)
    }
}

impl From<i64> for ValNumber {
    fn from(value: i64) -> Self {
        ValNumber::I64(value)
    }
}

impl From<u64> for ValNumber {
    fn from(value: u64) -> Self {
        ValNumber::U64(value)
    }
}

impl From<f32> for ValNumber {
    fn from(value: f32) -> Self {
        ValNumber::F32(value)
    }
}

impl From<f64> for ValNumber {
    fn from(value: f64) -> Self {
        ValNumber::F64(value)
    }
}

/// Partial ordering for `serde_json::Number` to allow us to compare numbers of different types.
impl PartialOrd for ValNumber {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (ValNumber::I32(left), ValNumber::I32(right)) => left.partial_cmp(right),
            (ValNumber::I32(left), ValNumber::I64(right)) => left.partial_cmp(&(*right as i32)),
            (ValNumber::I32(left), ValNumber::U64(right)) => left.partial_cmp(&(*right as i32)),
            (ValNumber::I32(left), ValNumber::F32(right)) => (*left as f32).partial_cmp(right),
            (ValNumber::I32(left), ValNumber::F64(right)) => (*left as f64).partial_cmp(right),

            (ValNumber::I64(left), ValNumber::I32(right)) => left.partial_cmp(&(*right as i64)),
            (ValNumber::I64(left), ValNumber::I64(right)) => left.partial_cmp(right),
            (ValNumber::I64(left), ValNumber::U64(right)) => left.partial_cmp(&(*right as i64)),
            (ValNumber::I64(left), ValNumber::F32(right)) => (*left as f32).partial_cmp(right),
            (ValNumber::I64(left), ValNumber::F64(right)) => (*left as f64).partial_cmp(right),

            (ValNumber::U64(left), ValNumber::I32(right)) => left.partial_cmp(&(*right as u64)),
            (ValNumber::U64(left), ValNumber::I64(right)) => left.partial_cmp(&(*right as u64)),
            (ValNumber::U64(left), ValNumber::U64(right)) => left.partial_cmp(right),
            (ValNumber::U64(left), ValNumber::F32(right)) => (*left as f32).partial_cmp(right),
            (ValNumber::U64(left), ValNumber::F64(right)) => (*left as f64).partial_cmp(right),

            (ValNumber::F32(left), ValNumber::I32(right)) => left.partial_cmp(&(*right as f32)),
            (ValNumber::F32(left), ValNumber::I64(right)) => left.partial_cmp(&(*right as f32)),
            (ValNumber::F32(left), ValNumber::U64(right)) => left.partial_cmp(&(*right as f32)),
            (ValNumber::F32(left), ValNumber::F32(right)) => left.partial_cmp(right),
            (ValNumber::F32(left), ValNumber::F64(right)) => (*left as f64).partial_cmp(right),

            (ValNumber::F64(left), ValNumber::I32(right)) => left.partial_cmp(&(*right as f64)),
            (ValNumber::F64(left), ValNumber::I64(right)) => left.partial_cmp(&(*right as f64)),
            (ValNumber::F64(left), ValNumber::U64(right)) => left.partial_cmp(&(*right as f64)),
            (ValNumber::F64(left), ValNumber::F32(right)) => left.partial_cmp(&(*right as f64)),
            (ValNumber::F64(left), ValNumber::F64(right)) => left.partial_cmp(right),
        }
    }
}

/// Represent a value that can be used in:
/// - arguments
/// - return values
/// - context values
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Val {
    Bool(bool),
    Number(ValNumber),
    String(String),
    List(Vec<Val>),
    Object(HashMap<String, Val>),
    Binary(bytes::Bytes),
    Enum(String),
    Null,
}

pub const TRUE: Val = Val::Bool(true);
pub const FALSE: Val = Val::Bool(false);

impl Val {
    pub fn get(&self, key: &str) -> Option<&Val> {
        match self {
            Val::Object(o) => o.get(key),
            _ => None,
        }
    }
}

impl Display for Val {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Val::Bool(b) => write!(f, "{b}"),
            Val::Number(n) => write!(f, "{n}"),
            Val::String(s) => write!(f, "\"{s}\""),
            Val::List(l) => {
                write!(f, "[")?;
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Val::Object(o) => {
                write!(f, "{{")?;
                for (i, (k, v)) in o.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
            Val::Binary(_) => write!(f, "Binary"),
            Val::Enum(e) => write!(f, "{e}"),
            Val::Null => write!(f, "null"),
        }
    }
}

impl TryInto<serde_json::Value> for Val {
    type Error = serde_json::Error;

    fn try_into(self) -> Result<serde_json::Value, Self::Error> {
        match self {
            Val::Null => Ok(serde_json::Value::Null),
            Val::Bool(b) => Ok(serde_json::Value::Bool(b)),
            Val::Number(n) => {
                Ok(serde_json::Value::Number(n.try_into().map_err(|_| {
                    serde_json::Error::custom("Invalid number")
                })?))
            }
            Val::String(s) => Ok(serde_json::Value::String(s)),
            Val::List(l) => Ok(serde_json::Value::Array(
                l.into_iter()
                    .map(|v| v.try_into())
                    .collect::<Result<_, _>>()?,
            )),
            Val::Object(o) => Ok(serde_json::Value::Object(
                o.into_iter()
                    .map(|(k, v)| Ok((k, v.try_into()?)))
                    .collect::<Result<_, _>>()?,
            )),
            Val::Enum(e) => Ok(serde_json::Value::String(e)),
            Val::Binary(_) => Err(Error::custom("Binary is not supported")),
        }
    }
}

impl TryFrom<ConstValue> for Val {
    type Error = serde_json::Error;

    fn try_from(value: ConstValue) -> Result<Self, Self::Error> {
        match value {
            ConstValue::Null => Ok(Val::Null),
            ConstValue::Boolean(b) => Ok(Val::Bool(b)),
            ConstValue::Number(n) => {
                Ok(Val::Number(n.try_into().map_err(|_| {
                    serde_json::Error::custom("Invalid number")
                })?))
            }
            ConstValue::String(s) => Ok(Val::String(s)),
            ConstValue::List(l) => Ok(Val::List(
                l.into_iter()
                    .map(|v| v.try_into())
                    .collect::<Result<_, _>>()?,
            )),
            ConstValue::Object(o) => Ok(Val::Object(
                o.into_iter()
                    .map(|(k, v)| Ok((k.to_string(), v.try_into()?)))
                    .collect::<Result<_, _>>()?,
            )),
            ConstValue::Binary(b) => Ok(Val::Binary(b)),
            ConstValue::Enum(e) => Ok(Val::Enum(e.to_string())),
        }
    }
}

impl From<serde_json::Value> for Val {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Val::Null,
            serde_json::Value::Bool(b) => Val::Bool(b),
            serde_json::Value::Number(n) => Val::Number(n.try_into().unwrap()),
            serde_json::Value::String(s) => Val::String(s),
            serde_json::Value::Array(l) => Val::List(l.into_iter().map(|v| v.into()).collect()),
            serde_json::Value::Object(o) => Val::Object(
                o.into_iter()
                    .map(|(k, v)| (k, v.into()))
                    .collect::<HashMap<_, _>>(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use multiplatform_test::multiplatform_test;

    #[multiplatform_test]
    fn test_number_eq() {
        let one_u64: ValNumber = ValNumber::from(1u64);
        let one_i64: ValNumber = ValNumber::from(1i64);
        let one_f64: ValNumber = ValNumber::from(1.0);

        let ones = vec![one_u64, one_i64, one_f64];

        for left in &ones {
            for right in &ones {
                assert!(left.partial_cmp(right) == Some(std::cmp::Ordering::Equal))
            }
        }
    }

    #[multiplatform_test]
    fn test_number_lt() {
        let min_u64 = ValNumber::from(u64::MIN);
        let min_i64 = ValNumber::from(i64::MIN);
        let min_f64 = ValNumber::from(f64::MIN);

        let max_u64 = ValNumber::from(u64::MAX);
        let max_i64 = ValNumber::from(i64::MAX);
        let max_f64 = ValNumber::from(f64::MAX);

        let mins = vec![min_u64, min_i64, min_f64];
        let maxs = vec![max_u64, max_i64, max_f64];

        // any min is less than any max
        for left in &mins {
            for right in &maxs {
                assert!(left.partial_cmp(right) == Some(std::cmp::Ordering::Less));
                assert!(right.partial_cmp(left) == Some(std::cmp::Ordering::Greater));
            }
        }
    }
}
