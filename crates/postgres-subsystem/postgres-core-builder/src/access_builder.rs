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
    ast::ast_types::{AstAccessExpr, AstAnnotationParams, AstLiteral},
    typechecker::Typed,
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct ResolvedAccess {
    pub default: Option<AstAccessExpr<Typed>>,

    pub query: Option<AstAccessExpr<Typed>>,

    pub mutation: Option<AstAccessExpr<Typed>>,
    pub creation: Option<AstAccessExpr<Typed>>,
    pub update: Option<AstAccessExpr<Typed>>,
    pub delete: Option<AstAccessExpr<Typed>>,
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
            None | Some(AstAccessExpr::Literal(AstLiteral::Boolean(false, _)))
        )
    }

    pub fn update_allowed(&self) -> bool {
        !matches!(
            self.update
                .as_ref()
                .or(self.mutation.as_ref())
                .or(self.default.as_ref()),
            None | Some(AstAccessExpr::Literal(AstLiteral::Boolean(false, _)))
        )
    }
}

pub fn build_access(
    access_annotation_params: Option<&AstAnnotationParams<Typed>>,
) -> ResolvedAccess {
    match access_annotation_params {
        Some(p) => match p {
            AstAnnotationParams::Single(default, _) => ResolvedAccess {
                default: Some(default.to_access_expr()),
                ..Default::default()
            },
            AstAnnotationParams::Map(m, _) => {
                let query = m.get("query").map(|p| p.to_access_expr());
                let mutation = m.get("mutation").map(|p| p.to_access_expr());
                let creation = m.get("create").map(|p| p.to_access_expr());
                let update = m.get("update").map(|p| p.to_access_expr());
                let delete = m.get("delete").map(|p| p.to_access_expr());

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
