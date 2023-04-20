// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::ast::ast_types::FieldSelection;

use super::{Type, Typed};

impl FieldSelection<Typed> {
    pub fn typ(&self) -> &Type {
        match &self {
            FieldSelection::Single(_, typ) => typ,
            FieldSelection::Select(_, _, _, typ) => typ,
        }
    }
}
