// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use indexmap::IndexMap;

use common::value::Val;

use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;

pub type Arguments = IndexMap<String, Val>;

pub fn find_arg<'a>(arguments: &'a Arguments, arg_name: &str) -> Option<&'a Val> {
    arguments.iter().find_map(|argument| {
        let (argument_name, argument_value) = argument;
        if arg_name == argument_name {
            Some(argument_value)
        } else {
            None
        }
    })
}

pub(crate) fn get_argument_field<'a>(argument_value: &'a Val, field_name: &str) -> Option<&'a Val> {
    match argument_value {
        Val::Object(value) => value.get(field_name),
        _ => None,
    }
}

pub(super) fn to_pg_vector(
    value: &Val,
    parameter_name: &str,
) -> Result<Vec<f32>, PostgresExecutionError> {
    let vec_value: Vec<f32> = match value {
        Val::List(vector) => vector
            .iter()
            .map(|v| match v {
                Val::Number(n) => Ok(n.as_f64().unwrap() as f32),
                _ => Err(PostgresExecutionError::Validation(
                    parameter_name.into(),
                    "Invalid vector parameter: element is not of float type".into(),
                )),
            })
            .collect(),
        _ => Err(PostgresExecutionError::Validation(
            parameter_name.into(),
            "Invalid vector parameter: must be a list of floats".into(),
        )),
    }?;

    Ok(vec_value)
}
