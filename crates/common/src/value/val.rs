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
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

/// Represent a value that can be used in:
/// - arguments
/// - return values
/// - context values
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Val {
    Bool(bool),
    Number(serde_json::Number),
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
    pub fn into_json(self) -> Result<serde_json::Value, serde_json::Error> {
        self.try_into()
    }

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
            Val::Number(n) => {
                if let Some(n) = n.as_f64() {
                    write!(f, "{n}")
                } else if let Some(n) = n.as_i64() {
                    write!(f, "{n}")
                } else if let Some(n) = n.as_u64() {
                    write!(f, "{n}")
                } else {
                    write!(f, "NaN")
                }
            }
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
            Val::Number(n) => Ok(serde_json::Value::Number(n)),
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

impl From<ConstValue> for Val {
    fn from(value: ConstValue) -> Self {
        match value {
            ConstValue::Null => Val::Null,
            ConstValue::Boolean(b) => Val::Bool(b),
            ConstValue::Number(n) => Val::Number(n),
            ConstValue::String(s) => Val::String(s),
            ConstValue::List(l) => Val::List(l.into_iter().map(|v| v.into()).collect()),
            ConstValue::Object(o) => Val::Object(
                o.into_iter()
                    .map(|(k, v)| (k.to_string(), v.into()))
                    .collect::<HashMap<_, _>>(),
            ),
            ConstValue::Binary(b) => Val::Binary(b),
            ConstValue::Enum(e) => Val::Enum(e.to_string()),
        }
    }
}

impl From<serde_json::Value> for Val {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Val::Null,
            serde_json::Value::Bool(b) => Val::Bool(b),
            serde_json::Value::Number(n) => Val::Number(n),
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

impl Serialize for Val {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Val::Null => serializer.serialize_none(),
            Val::Bool(b) => serializer.serialize_bool(*b),
            Val::Number(n) => {
                if let Some(n) = n.as_f64() {
                    serializer.serialize_f64(n)
                } else if let Some(n) = n.as_i64() {
                    serializer.serialize_i64(n)
                } else if let Some(n) = n.as_u64() {
                    serializer.serialize_u64(n)
                } else {
                    serializer.serialize_f64(0.0)
                }
            }
            Val::String(s) => serializer.serialize_str(s),
            Val::List(l) => {
                let mut seq = serializer.serialize_seq(Some(l.len()))?;
                for e in l {
                    seq.serialize_element(e)?;
                }
                seq.end()
            }
            Val::Object(o) => {
                let mut map = serializer.serialize_map(Some(o.len()))?;
                for (k, v) in o {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            Val::Enum(e) => serializer.serialize_str(e),
            Val::Binary(_) => Err(serde::ser::Error::custom("Binary is not supported")),
        }
    }
}
