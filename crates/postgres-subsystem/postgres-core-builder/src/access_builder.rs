// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

use core_model_builder::{
    ast::ast_types::{AstAnnotationParams, AstExpr},
    typechecker::Typed,
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct ResolvedAccess {
    pub default: Option<AstExpr<Typed>>,

    pub query: Option<AstExpr<Typed>>,

    pub mutation: Option<AstExpr<Typed>>,
    pub creation: Option<AstExpr<Typed>>,
    pub update: Option<AstExpr<Typed>>,
    pub delete: Option<AstExpr<Typed>>,
}

impl ResolvedAccess {
    // The annotation parameter hierarchy is:
    // value -> query
    //       -> mutation -> create
    //                   -> update
    //                   -> delete
    // Any lower node in the hierarchy get a priority over its parent.

    pub fn creation_allowed(&self) -> bool {
        !matches!(
            self.creation
                .as_ref()
                .or(self.mutation.as_ref())
                .or(self.default.as_ref()),
            None | Some(AstExpr::BooleanLiteral(false, _))
        )
    }

    pub fn update_allowed(&self) -> bool {
        !matches!(
            self.update
                .as_ref()
                .or(self.mutation.as_ref())
                .or(self.default.as_ref()),
            None | Some(AstExpr::BooleanLiteral(false, _))
        )
    }
}

pub fn build_access(
    access_annotation_params: Option<&AstAnnotationParams<Typed>>,
) -> ResolvedAccess {
    match access_annotation_params {
        Some(p) => match p {
            AstAnnotationParams::Single(default, _) => ResolvedAccess {
                default: Some(default.clone()),
                ..Default::default()
            },
            AstAnnotationParams::Map(m, _) => {
                let query = m.get("query").cloned();
                let mutation = m.get("mutation").cloned();
                let creation = m.get("create").cloned();
                let update = m.get("update").cloned();
                let delete = m.get("delete").cloned();

                ResolvedAccess {
                    default: None,
                    query,
                    mutation,
                    creation,
                    update,
                    delete,
                }
            }
            _ => panic!(),
        },
        None => ResolvedAccess::default(),
    }
}
