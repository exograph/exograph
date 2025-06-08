// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_value::{Name, indexmap::IndexMap};
use serde::Serialize;

use common::value::Val;

#[derive(Debug, Serialize)]
pub struct ValidatedField {
    pub alias: Option<Name>,
    /// The name of the field.
    pub name: Name,
    /// The arguments to the field, empty if no arguments are provided.
    pub arguments: IndexMap<String, Val>,

    /// The subfields being selected in this field, if it is an object. Empty if no fields are
    /// being selected.
    pub subfields: Vec<ValidatedField>,
}

impl ValidatedField {
    pub fn output_name(&self) -> String {
        self.alias.as_ref().unwrap_or(&self.name).to_string()
    }
}
