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
use core_model_builder::typechecker::Typed;
use core_model_builder::typechecker::annotation::AnnotationSpec;

use crate::ast::ast_types::{
    AstFieldDefault, AstFieldDefaultKind, AstFieldDefaultValue, FieldSelection, Untyped,
};

use super::{Scope, Type, TypecheckFrom};

impl TypecheckFrom<AstFieldDefaultValue<Untyped>> for AstFieldDefaultValue<Typed> {
    fn shallow(untyped: &AstFieldDefaultValue<Untyped>) -> AstFieldDefaultValue<Typed> {
        match untyped {
            AstFieldDefaultValue::Literal(lit) => AstFieldDefaultValue::Literal(lit.clone()),
            AstFieldDefaultValue::FieldSelection(sel) => {
                AstFieldDefaultValue::FieldSelection(FieldSelection::shallow(sel))
            }
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
            AstFieldDefaultValue::Literal(_) => false,
            AstFieldDefaultValue::FieldSelection(sel) => {
                sel.pass(type_env, annotation_env, scope, errors)
            }
        }
    }
}

impl TypecheckFrom<AstFieldDefault<Untyped>> for AstFieldDefault<Typed> {
    fn shallow(untyped: &AstFieldDefault<Untyped>) -> AstFieldDefault<Typed> {
        let kind = {
            match &untyped.kind {
                AstFieldDefaultKind::Function(fn_name, args) => {
                    AstFieldDefaultKind::Function(fn_name.clone(), args.clone())
                }
                AstFieldDefaultKind::Value(value) => {
                    AstFieldDefaultKind::Value(AstFieldDefaultValue::shallow(value))
                }
            }
        };

        AstFieldDefault {
            kind,
            span: untyped.span,
        }
    }

    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        match &mut self.kind {
            AstFieldDefaultKind::Function(_, _) => false, // literals don't need typechecking
            AstFieldDefaultKind::Value(value) => {
                value.pass(type_env, annotation_env, scope, errors)
            }
        }
    }
}
