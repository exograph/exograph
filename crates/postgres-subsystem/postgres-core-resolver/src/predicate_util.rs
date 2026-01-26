// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Shared utilities for predicate mapping between GraphQL and RPC resolvers.

use common::value::Val;
use common::value::val::ValNumber;
use exo_sql::{CaseSensitivity, ParamEquality, Predicate};

use crate::postgres_execution_error::PostgresExecutionError;

/// Get a field from an object-typed Val
pub fn get_argument_field<'a>(argument: &'a Val, field_name: &str) -> Option<&'a Val> {
    match argument {
        Val::Object(map) => map.get(field_name),
        _ => None,
    }
}

/// Convert a Val list to a Vec<f32> for vector operations
pub fn to_pg_vector(value: &Val, param_name: &str) -> Result<Vec<f32>, PostgresExecutionError> {
    match value {
        Val::List(list) => {
            let mut result = Vec::with_capacity(list.len());
            for v in list {
                match v {
                    Val::Number(n) => {
                        result.push(n.as_f64().unwrap() as f32);
                    }
                    _ => {
                        return Err(PostgresExecutionError::Validation(
                            param_name.into(),
                            "Vector values must be numbers".into(),
                        ));
                    }
                }
            }
            Ok(result)
        }
        _ => Err(PostgresExecutionError::Validation(
            param_name.into(),
            "Vector value must be an array".into(),
        )),
    }
}

/// Map predicate from operation name to a Predicate
pub(crate) fn predicate_from_name<C: PartialEq + ParamEquality>(
    op_name: &str,
    lhs: C,
    rhs: C,
) -> Result<Predicate<C>, PostgresExecutionError> {
    match op_name {
        "eq" => Ok(Predicate::Eq(lhs, rhs)),
        "neq" => Ok(Predicate::Neq(lhs, rhs)),
        "lt" => Ok(Predicate::Lt(lhs, rhs)),
        "lte" => Ok(Predicate::Lte(lhs, rhs)),
        "gt" => Ok(Predicate::Gt(lhs, rhs)),
        "gte" => Ok(Predicate::Gte(lhs, rhs)),
        "like" => Ok(Predicate::StringLike(lhs, rhs, CaseSensitivity::Sensitive)),
        "ilike" => Ok(Predicate::StringLike(
            lhs,
            rhs,
            CaseSensitivity::Insensitive,
        )),
        "startsWith" => Ok(Predicate::StringStartsWith(lhs, rhs)),
        "endsWith" => Ok(Predicate::StringEndsWith(lhs, rhs)),
        "contains" => Ok(Predicate::JsonContains(lhs, rhs)),
        "containedBy" => Ok(Predicate::JsonContainedBy(lhs, rhs)),
        "matchKey" => Ok(Predicate::JsonMatchKey(lhs, rhs)),
        "matchAnyKey" => Ok(Predicate::JsonMatchAnyKey(lhs, rhs)),
        "matchAllKeys" => Ok(Predicate::JsonMatchAllKeys(lhs, rhs)),
        _ => Err(PostgresExecutionError::Validation(
            op_name.into(),
            format!("Unknown predicate operator: {op_name}"),
        )),
    }
}

/// Convert serde_json::Value to common::value::Val
pub fn json_to_val(json: &serde_json::Value) -> Val {
    match json {
        serde_json::Value::Null => Val::Null,
        serde_json::Value::Bool(b) => Val::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Val::Number(ValNumber::I64(i))
            } else if let Some(f) = n.as_f64() {
                Val::Number(ValNumber::F64(f))
            } else {
                Val::Null
            }
        }
        serde_json::Value::String(s) => Val::String(s.clone()),
        serde_json::Value::Array(arr) => Val::List(arr.iter().map(json_to_val).collect()),
        serde_json::Value::Object(obj) => Val::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), json_to_val(v)))
                .collect(),
        ),
    }
}
