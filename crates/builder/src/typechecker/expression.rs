// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use codemap_diagnostic::Diagnostic;
use core_model::mapped_arena::MappedArena;
use core_model_builder::typechecker::{Typed, annotation::AnnotationSpec};

use crate::ast::ast_types::{AstAccessExpr, FieldSelection, LogicalOp, RelationalOp, Untyped};

use super::{Scope, Type, TypecheckFrom};

impl TypecheckFrom<AstAccessExpr<Untyped>> for AstAccessExpr<Typed> {
    fn shallow(untyped: &AstAccessExpr<Untyped>) -> AstAccessExpr<Typed> {
        match untyped {
            AstAccessExpr::FieldSelection(select) => {
                AstAccessExpr::FieldSelection(FieldSelection::shallow(select))
            }
            AstAccessExpr::LogicalOp(logic) => AstAccessExpr::LogicalOp(LogicalOp::shallow(logic)),
            AstAccessExpr::RelationalOp(relation) => {
                AstAccessExpr::RelationalOp(RelationalOp::shallow(relation))
            }
            AstAccessExpr::Literal(lit) => AstAccessExpr::Literal(lit.clone()),
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        match self {
            AstAccessExpr::FieldSelection(select) => {
                select.pass(type_env, annotation_env, scope, errors)
            }
            AstAccessExpr::LogicalOp(logic) => logic.pass(type_env, annotation_env, scope, errors),
            AstAccessExpr::RelationalOp(relation) => {
                relation.pass(type_env, annotation_env, scope, errors)
            }
            AstAccessExpr::Literal(_) => false,
        }
    }
}
