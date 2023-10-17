// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub mod annotation;
pub mod annotation_map;
use std::collections::HashMap;

pub use annotation_map::AnnotationMap;
pub mod typ;

mod expression;
mod field_type;
mod logical_op;
mod selection;

pub(super) use typ::Type;

use crate::ast::ast_types::NodeTypedness;

use serde::{Deserialize, Serialize};

/// The scope for the current typechecking context.
///
/// Typically starts out with ("self" -> name of the type being checked) extended with additional
/// mappings when a access control selection with a function such as `.some(du => ...)` is
/// encountered.
#[derive(Debug, Default)]
pub struct Scope {
    mapping: HashMap<String, String>,
}

impl Scope {
    pub fn with_enclosing_type(enclosing_type: String) -> Self {
        Self {
            mapping: HashMap::from_iter([("self".to_string(), enclosing_type)]),
        }
    }

    pub fn with_additional_mapping(&self, additional_mapping: HashMap<String, String>) -> Self {
        Self {
            mapping: self
                .mapping
                .clone()
                .into_iter()
                .chain(additional_mapping)
                .collect(),
        }
    }

    pub fn get_type(&self, name: &str) -> Option<&str> {
        self.mapping.get(name).map(|s| s.as_str())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Typed;
impl NodeTypedness for Typed {
    type FieldSelection = Type;
    type RelationalOp = Type;
    type Expr = Type;
    type LogicalOp = Type;
    type Field = Type;
    type Annotations = AnnotationMap;
    type Type = bool;
}
