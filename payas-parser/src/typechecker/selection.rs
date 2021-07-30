use anyhow::Result;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::{
    ast::ast_types::{FieldSelection, Identifier},
    typechecker::typ::CompositeTypeKind,
};

use super::{Scope, Type, Typecheck};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TypedFieldSelection {
    Single(Identifier, Type),
    Select(Box<TypedFieldSelection>, Identifier, Type),
}

impl TypedFieldSelection {
    pub fn typ(&self) -> &Type {
        match &self {
            TypedFieldSelection::Single(_, typ) => typ,
            TypedFieldSelection::Select(_, _, typ) => typ,
        }
    }
}

impl Typecheck<TypedFieldSelection> for FieldSelection {
    fn shallow(
        &self,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<TypedFieldSelection> {
        Ok(match &self {
            FieldSelection::Single(v) => TypedFieldSelection::Single(v.clone(), Type::Defer),
            FieldSelection::Select(selection, i, _) => TypedFieldSelection::Select(
                Box::new(selection.shallow(errors)?),
                i.clone(),
                Type::Defer,
            ),
        })
    }

    fn pass(
        &self,
        typ: &mut TypedFieldSelection,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match &self {
            FieldSelection::Single(Identifier(i, s)) => {
                if typ.typ().is_incomplete() {
                    if i.as_str() == "self" {
                        if let Some(enclosing) = &scope.enclosing_model {
                            *typ = TypedFieldSelection::Single(
                                Identifier(i.clone(), *s),
                                Type::Reference(enclosing.clone()),
                            );
                            true
                        } else {
                            *typ =
                                TypedFieldSelection::Single(Identifier(i.clone(), *s), Type::Error);

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
                            *typ = TypedFieldSelection::Single(
                                Identifier(i.clone(), *s),
                                Type::Reference(context_type.name.clone()),
                            );
                        } else {
                            *typ =
                                TypedFieldSelection::Single(Identifier(i.clone(), *s), Type::Error);

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
            FieldSelection::Select(selection, i, _) => {
                if let TypedFieldSelection::Select(prefix, _, typ) = typ {
                    let in_updated = selection.pass(prefix, env, scope, errors);
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
                                        span: *selection.span(),
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
                } else {
                    panic!()
                }
            }
        }
    }
}
