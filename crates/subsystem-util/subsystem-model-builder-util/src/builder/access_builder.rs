// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use codemap::{CodeMap, Span};
use core_model_builder::{
    ast::ast_types::{AstAnnotationParams, AstExpr},
    typechecker::Typed,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedAccess {
    pub value: AstExpr<Typed>,
}

impl ResolvedAccess {
    fn restrictive() -> Self {
        ResolvedAccess {
            value: AstExpr::BooleanLiteral(false, null_span()),
        }
    }
}

fn null_span() -> Span {
    let mut codemap = CodeMap::new();
    let file = codemap.add_file("".to_string(), "".to_string());
    file.span
}

pub fn build_access(
    access_annotation_params: Option<&AstAnnotationParams<Typed>>,
) -> ResolvedAccess {
    match access_annotation_params {
        Some(p) => {
            let value = match p {
                AstAnnotationParams::Single(default, _) => default,
                // For Map format annotations (e.g., @access(query=true, mutation=false)),
                // return restrictive access. These types are typically from @postgres modules
                // included for type generation, and their actual access control is handled
                // by the postgres subsystem.
                AstAnnotationParams::Map(_, _) => return ResolvedAccess::restrictive(),
                _ => panic!(), // module queries and annotations should only have a single parameter (the default value)
            };

            ResolvedAccess {
                value: value.clone(),
            }
        }
        None => ResolvedAccess::restrictive(),
    }
}
