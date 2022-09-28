use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_core_model::mapped_arena::MappedArena;
use payas_core_model_builder::typechecker::annotation::AnnotationSpec;
use payas_core_model_builder::typechecker::Typed;

use crate::ast::ast_types::{AstExpr, AstFieldDefault, AstFieldDefaultKind, Untyped};
use payas_database_model_builder::builder::{
    DEFAULT_FN_AUTOINCREMENT, DEFAULT_FN_CURRENT_TIME, DEFAULT_FN_GENERATE_UUID,
};

use super::{Scope, Type, TypecheckFrom};

impl TypecheckFrom<AstFieldDefault<Untyped>> for AstFieldDefault<Typed> {
    fn shallow(untyped: &AstFieldDefault<Untyped>) -> AstFieldDefault<Typed> {
        let kind = {
            match &untyped.kind {
                AstFieldDefaultKind::Function(fn_name, args) => AstFieldDefaultKind::Function(
                    fn_name.clone(),
                    args.iter().map(AstExpr::shallow).collect(),
                ),
                AstFieldDefaultKind::Value(expr) => {
                    AstFieldDefaultKind::Value(AstExpr::shallow(expr))
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
        let mut check_literal = |expr: &mut AstExpr<Typed>| match expr {
            AstExpr::BooleanLiteral(_, _)
            | AstExpr::StringLiteral(_, _)
            | AstExpr::NumberLiteral(_, _) => expr.pass(type_env, annotation_env, scope, errors),

            _ => {
                errors.push(Diagnostic {
                    level: Level::Error,
                    message: "Must be a literal.".to_string(),
                    code: Some("C000".to_string()),
                    spans: vec![SpanLabel {
                        span: self.span,
                        style: SpanStyle::Primary,
                        label: Some("not a literal".to_string()),
                    }],
                });
                false
            }
        };

        match &mut self.kind {
            AstFieldDefaultKind::Function(fn_name, args) => {
                let args_changed = args.iter_mut().any(check_literal);

                match fn_name.as_str() {
                    DEFAULT_FN_AUTOINCREMENT
                    | DEFAULT_FN_CURRENT_TIME
                    | DEFAULT_FN_GENERATE_UUID => {}
                    _ => {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "Unknown kind of default value specified: {}",
                                fn_name
                            ),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: self.span,
                                style: SpanStyle::Primary,
                                label: Some("unknown kind".to_string()),
                            }],
                        });
                    }
                };

                args_changed
            }
            AstFieldDefaultKind::Value(expr) => check_literal(expr),
        }
    }
}
