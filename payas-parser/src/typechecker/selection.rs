use anyhow::Result;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;

use crate::{
    ast::ast_types::{FieldSelection, Identifier, Untyped},
    typechecker::typ::CompositeTypeKind,
};

use super::{Scope, Type, TypecheckNew, Typed};

impl FieldSelection<Typed> {
    pub fn typ(&self) -> &Type {
        match &self {
            FieldSelection::Single(_, typ) => typ,
            FieldSelection::Select(_, _, _, typ) => typ,
        }
    }
}

impl TypecheckNew<FieldSelection<Untyped>> for FieldSelection<Typed> {
    fn shallow(
        untyped: &FieldSelection<Untyped>,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<FieldSelection<Typed>> {
        Ok(match untyped {
            FieldSelection::Single(v, _) => FieldSelection::Single(v.clone(), Type::Defer),
            FieldSelection::Select(selection, i, span, _) => FieldSelection::Select(
                Box::new(FieldSelection::shallow(selection, errors)?),
                i.clone(),
                *span,
                Type::Defer,
            ),
        })
    }

    fn pass(
        &mut self,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match self {
            FieldSelection::Single(Identifier(i, s), typ) => {
                if typ.is_incomplete() {
                    if i.as_str() == "self" {
                        if let Some(enclosing) = &scope.enclosing_model {
                            *typ = Type::Reference(enclosing.clone());
                            true
                        } else {
                            *typ = Type::Error;

                            errors.push(Diagnostic {
                                level: Level::Error,
                                message: "Cannot use self outside a model".to_string(),
                                code: Some("C000".to_string()),
                                spans: vec![SpanLabel {
                                    span: *s,
                                    style: SpanStyle::Primary,
                                    label: Some("self not allowed".to_string()),
                                }],
                            });

                            false
                        }
                    } else {
                        let context_type = env.get_by_key(i).and_then(|t| match t {
                            Type::Composite(c) if c.kind == CompositeTypeKind::Context => Some(c),
                            _ => None,
                        });

                        if let Some(context_type) = context_type {
                            *typ = Type::Reference(context_type.name.clone());
                        } else {
                            *typ = Type::Error;

                            errors.push(Diagnostic {
                                level: Level::Error,
                                message: format!("Reference to unknown context: {}", i),
                                code: Some("C000".to_string()),
                                spans: vec![SpanLabel {
                                    span: *s,
                                    style: SpanStyle::Primary,
                                    label: Some("unknown context".to_string()),
                                }],
                            });
                        }
                        false
                    }
                } else {
                    false
                }
            }
            FieldSelection::Select(prefix, i, _, typ) => {
                let in_updated = prefix.pass(env, scope, errors);
                let out_updated = if typ.is_incomplete() {
                    if let Type::Composite(c) = prefix.typ().deref(env) {
                        if let Some(field) = c.fields.iter().find(|f| f.name == i.0) {
                            if !field.typ.is_incomplete() {
                                *typ = field.typ.clone();
                                true
                            } else {
                                *typ = Type::Error;
                                // no diagnostic because the prefix is incomplete
                                false
                            }
                        } else {
                            *typ = Type::Error;
                            errors.push(Diagnostic {
                                level: Level::Error,
                                message: format!("No such field {} on type {}", i.0, c.name),
                                code: Some("C000".to_string()),
                                spans: vec![SpanLabel {
                                    span: i.1,
                                    style: SpanStyle::Primary,
                                    label: Some("unknown field".to_string()),
                                }],
                            });
                            false
                        }
                    } else {
                        *typ = Type::Error;

                        if !prefix.typ().is_error() {
                            errors.push(Diagnostic {
                                level: Level::Error,
                                message: format!(
                                    "Cannot read field {} from a non-composite type {}",
                                    i.0,
                                    prefix.typ().deref(env)
                                ),
                                code: Some("C000".to_string()),
                                spans: vec![SpanLabel {
                                    span: *prefix.span(),
                                    style: SpanStyle::Primary,
                                    label: Some("non-composite value".to_string()),
                                }],
                            });
                        }

                        false
                    }
                } else {
                    false
                };

                in_updated || out_updated
            }
        }
    }
}
