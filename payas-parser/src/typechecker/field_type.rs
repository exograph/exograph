use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::AstFieldType;

use super::annotation::AnnotationSpec;
use super::{Scope, Type, TypecheckInto};

impl TypecheckInto<Type> for AstFieldType {
    fn shallow(&self) -> Type {
        match &self {
            AstFieldType::Plain(_, _) => Type::Defer,
            AstFieldType::Optional(u) => Type::Optional(Box::new(u.shallow())),
            AstFieldType::List(u) => Type::List(Box::new(u.shallow())),
        }
    }

    fn pass(
        &self,
        typ: &mut Type,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        if typ.is_incomplete() {
            match &self {
                AstFieldType::Plain(name, s) => {
                    if type_env.get_id(name.as_str()).is_some() {
                        *typ = Type::Reference(name.clone());
                        true
                    } else {
                        *typ = Type::Error;
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!("Reference to unknown type: {}", name),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: *s,
                                style: SpanStyle::Primary,
                                label: Some("unknown type".to_string()),
                            }],
                        });

                        false
                    }
                }

                AstFieldType::Optional(inner_ast) => {
                    if let Type::Optional(inner_typ) = typ {
                        inner_ast.pass(inner_typ, type_env, annotation_env, scope, errors)
                    } else {
                        panic!()
                    }
                }

                AstFieldType::List(inner_ast) => {
                    if let Type::List(inner_typ) = typ {
                        inner_ast.pass(inner_typ, type_env, annotation_env, scope, errors)
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
