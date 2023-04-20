// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_parser::types::{BaseType, Type};
use async_graphql_value::Name;

/// Validate the query payload.
///
/// Take a user submitted query along with the operation name and variables (from the request payload)
/// and transform the query into a validated form (in the process, validate the query).
pub mod operation;

pub mod document_validator;

mod arguments_validator;
mod operation_validator;
mod selection_set_validator;

pub fn underlying_type(typ: &Type) -> &Name {
    match &typ.base {
        BaseType::Named(name) => name,
        BaseType::List(typ) => underlying_type(typ),
    }
}

pub mod field;
pub mod validation_error;
