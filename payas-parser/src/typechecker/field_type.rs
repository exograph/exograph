use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::AstFieldType;

use super::{Scope, Type, Typecheck};

impl Typecheck<Type> for AstFieldType {
    fn shallow(&self) -> Type {
        match &self {
            AstFieldType::Plain(_, _) => Type::Defer,
            AstFieldType::Optional(u) => Type::Optional(Box::new(u.shallow())),
            AstFieldType::List(u) => Type::List(Box::new(u.shallow())),
        }
    }

    fn pass(&self, typ: &mut Type, env: &MappedArena<Type>, scope: &Scope, errors: &mut Vec< codemap_diagnostic::Diagnostic>) -> bool {
        if typ.is_incomplete() {
            match &self {
                AstFieldType::Plain(name, s) => {
                    if env.get_id(name.as_str()).is_some() {
                        *typ = Type::Reference(name.clone());
                        true
                    } else {
                        *typ = Type::Error;
                        errors.push(
                            Diagnostic {
                                level: Level::Error,
                                message: format!("Reference to unknown type: {}", name),
                                code: Some("C000".to_string()),
                                spans: vec![
                                    SpanLabel {
                                        span: s.clone(),
                                        style: SpanStyle::Primary,
                                        label: Some("unknown type".to_string())
                                    }
                                ]
                            }
                        );

                        false
                    }
                }

                AstFieldType::Optional(inner_ast) => {
                    if let Type::Optional(inner_typ) = typ {
                        inner_ast.pass(inner_typ, env, scope, errors)
                    } else {
                        panic!()
                    }
                }

                AstFieldType::List(inner_ast) => {
                    if let Type::List(inner_typ) = typ {
                        inner_ast.pass(inner_typ, env, scope, errors)
                    } else {
                        panic!()
                    }
                }
            }
        } else {
            false
        }
    }
}
