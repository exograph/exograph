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

use crate::ast::ast_types::{AstExpr, FieldSelection, LogicalOp, RelationalOp, Untyped};

use super::{Scope, Type, TypecheckFrom};

impl TypecheckFrom<AstExpr<Untyped>> for AstExpr<Typed> {
    fn shallow(untyped: &AstExpr<Untyped>) -> AstExpr<Typed> {
        match untyped {
            AstExpr::FieldSelection(select) => {
                AstExpr::FieldSelection(FieldSelection::shallow(select))
            }
            AstExpr::LogicalOp(logic) => AstExpr::LogicalOp(LogicalOp::shallow(logic)),
            AstExpr::RelationalOp(relation) => {
                AstExpr::RelationalOp(RelationalOp::shallow(relation))
            }
            AstExpr::StringLiteral(v, s) => AstExpr::StringLiteral(v.clone(), *s),
            AstExpr::BooleanLiteral(v, s) => AstExpr::BooleanLiteral(*v, *s),
            AstExpr::NumberLiteral(v, s) => AstExpr::NumberLiteral(v.clone(), *s),
            AstExpr::StringList(v, s) => AstExpr::StringList(v.clone(), s.clone()),
            AstExpr::NullLiteral(s) => AstExpr::NullLiteral(*s),
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
            AstExpr::FieldSelection(select) => select.pass(type_env, annotation_env, scope, errors),
            AstExpr::LogicalOp(logic) => logic.pass(type_env, annotation_env, scope, errors),
            AstExpr::RelationalOp(relation) => {
                relation.pass(type_env, annotation_env, scope, errors)
            }
            AstExpr::StringList(_, _)
            | AstExpr::StringLiteral(_, _)
            | AstExpr::BooleanLiteral(_, _)
            | AstExpr::NumberLiteral(_, _)
            | AstExpr::NullLiteral(_) => false,
        }
    }
}
