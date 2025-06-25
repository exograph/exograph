// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::mapped_arena::MappedArena;
use core_model_builder::typechecker::Typed;
use core_model_builder::typechecker::annotation::{AnnotationSpec, AnnotationTarget};
use core_model_builder::typechecker::annotation_map::AnnotationMap;

use crate::ast::ast_types::{
    AstExpr, AstField, AstFieldDefault, AstFieldDefaultKind, AstFieldType, Untyped,
};

use super::annotation_map::AnnotationMapImpl;
use super::{Scope, Type, TypecheckFrom};

impl TypecheckFrom<AstField<Untyped>> for AstField<Typed> {
    fn shallow(untyped: &AstField<Untyped>) -> AstField<Typed> {
        let annotation_map = AnnotationMap::new(&untyped.annotations);

        AstField {
            name: untyped.name.clone(),
            typ: AstFieldType::shallow(&untyped.typ),
            annotations: annotation_map,
            doc_comments: untyped.doc_comments.clone(),
            default_value: untyped.default_value.as_ref().map(AstFieldDefault::shallow),
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
        let typ_changed = self.typ.pass(type_env, annotation_env, scope, errors);

        let annot_changed = self.annotations.pass(
            AnnotationTarget::Field,
            type_env,
            annotation_env,
            scope,
            errors,
        );

        let default_value_changed = self
            .default_value
            .as_mut()
            .map(|default_value| default_value.pass(type_env, annotation_env, scope, errors))
            .unwrap_or(false);

        if let Some(AstFieldDefault {
            kind: AstFieldDefaultKind::Value(expr),
            ..
        }) = &self.default_value
        {
            let type_name = self.typ.name();
            let mut assert_type = |types_allowed: &[&str]| {
                if !types_allowed.contains(&type_name.as_str()) {
                    let types_allowed: String = types_allowed.join(", ");

                    errors.push(Diagnostic {
                        level: Level::Error,
                        message:
                            "Literal specified for default value is not a valid type for field."
                                .to_string(),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: expr.span(),
                            style: SpanStyle::Primary,
                            label: Some(format!("should be of type {types_allowed}")),
                        }],
                    });
                }
            };

            match *expr {
                AstExpr::StringLiteral(_, _) => assert_type(&["String", "Decimal"]),
                AstExpr::BooleanLiteral(_, _) => assert_type(&["Boolean"]),
                AstExpr::NumberLiteral(_, _) => assert_type(&["Int", "Float"]),
                AstExpr::FieldSelection(_) => {
                    // no type-checking here, since we don't have enough information.
                    // For example `user: User = AuthContext.id` should check that `AuthContext.id`
                    // is of the same type as `User`'s primary key type, but we don't know that here.
                }

                _ => {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: "Non-literal specified in default value field.".to_string(),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: expr.span(),
                            style: SpanStyle::Primary,
                            label: Some("should be string, boolean, or a number".to_string()),
                        }],
                    });
                }
            }
        };

        typ_changed || annot_changed || default_value_changed
    }
}
