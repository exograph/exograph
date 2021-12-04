use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstFieldType, Untyped};

use super::annotation::AnnotationSpec;
use super::{Scope, Type, TypecheckFrom, Typed};

impl AstFieldType<Typed> {
    pub fn get_underlying_typename(&self, types: &MappedArena<Type>) -> Option<String> {
        match &self {
            AstFieldType::Plain(_, _, _, _) => self.to_typ(types).get_underlying_typename(types),
            AstFieldType::Optional(underlying) => underlying.get_underlying_typename(types),
        }
    }

    pub fn to_typ(&self, types: &MappedArena<Type>) -> Type {
        match &self {
            AstFieldType::Plain(name, params, ok, _) => {
                if !ok {
                    Type::Error
                } else {
                    match name.as_str() {
                        "Set" => Type::Set(Box::new(params[0].to_typ(types))),
                        "Array" => Type::Array(Box::new(params[0].to_typ(types))),
                        o => Type::Reference(types.get_id(o).unwrap()),
                    }
                }
            }
            AstFieldType::Optional(underlying) => {
                Type::Optional(Box::new(underlying.to_typ(types)))
            }
        }
    }
}

impl TypecheckFrom<AstFieldType<Untyped>> for AstFieldType<Typed> {
    fn shallow(untyped: &AstFieldType<Untyped>) -> AstFieldType<Typed> {
        match untyped {
            AstFieldType::Plain(name, params, _, s) => AstFieldType::Plain(
                name.clone(),
                params.iter().map(AstFieldType::shallow).collect(),
                false,
                *s,
            ),
            AstFieldType::Optional(u) => AstFieldType::Optional(Box::new(AstFieldType::shallow(u))),
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
            AstFieldType::Plain(name, params, ok, s) => {
                let ref_updated = if !*ok {
                    if type_env.get_id(name.as_str()).is_some()
                        || name.as_str() == "Set"
                        || name.as_str() == "Array"
                    {
                        *ok = true;
                        true
                    } else {
                        *ok = false;
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
                } else {
                    false
                };

                let params_updated = params
                    .iter_mut()
                    .map(|i| i.pass(type_env, annotation_env, scope, errors))
                    .filter(|b| *b)
                    .count()
                    > 0;

                ref_updated || params_updated
            }

            AstFieldType::Optional(inner) => inner.pass(type_env, annotation_env, scope, errors),
        }
    }
}
