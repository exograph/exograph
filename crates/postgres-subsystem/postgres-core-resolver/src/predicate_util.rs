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

use exo_sql_pg::{CaseSensitivity, PgAbstractPredicate, PgColumnPath};

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

/// Map predicate from operation name to an abstract predicate
pub(crate) fn predicate_from_name(
    op_name: &str,
    lhs: PgColumnPath,
    rhs: PgColumnPath,
) -> Result<PgAbstractPredicate, PostgresExecutionError> {
    match op_name {
        "eq" => Ok(PgAbstractPredicate::Eq(lhs, rhs)),
        "neq" => Ok(PgAbstractPredicate::Neq(lhs, rhs)),
        "lt" => Ok(PgAbstractPredicate::Lt(lhs, rhs)),
        "lte" => Ok(PgAbstractPredicate::Lte(lhs, rhs)),
        "gt" => Ok(PgAbstractPredicate::Gt(lhs, rhs)),
        "gte" => Ok(PgAbstractPredicate::Gte(lhs, rhs)),
        "like" => Ok(PgAbstractPredicate::StringLike(
            lhs,
            rhs,
            CaseSensitivity::Sensitive,
        )),
        "ilike" => Ok(PgAbstractPredicate::StringLike(
            lhs,
            rhs,
            CaseSensitivity::Insensitive,
        )),
        "startsWith" => Ok(PgAbstractPredicate::StringStartsWith(lhs, rhs)),
        "endsWith" => Ok(PgAbstractPredicate::StringEndsWith(lhs, rhs)),
        "contains" => Ok(PgAbstractPredicate::JsonContains(lhs, rhs)),
        "containedBy" => Ok(PgAbstractPredicate::JsonContainedBy(lhs, rhs)),
        "matchKey" => Ok(PgAbstractPredicate::JsonMatchKey(lhs, rhs)),
        "matchAnyKey" => Ok(PgAbstractPredicate::JsonMatchAnyKey(lhs, rhs)),
        "matchAllKeys" => Ok(PgAbstractPredicate::JsonMatchAllKeys(lhs, rhs)),
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
