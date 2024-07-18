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
use core_model_builder::typechecker::{annotation::AnnotationSpec, Typed};

use crate::ast::ast_types::{AstFieldType, Untyped};

use super::{Scope, Type, TypecheckFrom};

impl TypecheckFrom<AstFieldType<Untyped>> for AstFieldType<Typed> {
    fn shallow(untyped: &AstFieldType<Untyped>) -> AstFieldType<Typed> {
        match untyped {
            AstFieldType::Plain(module, name, params, _, s) => AstFieldType::Plain(
                module.clone(),
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
        _annotation_env: &HashMap<String, AnnotationSpec>,
        _scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool {
        match self {
            AstFieldType::Plain(_, name, params, ok, s) => {
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
                            message: format!("Reference to unknown type: {name}"),
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
                    .map(|i| i.pass(type_env, _annotation_env, _scope, errors))
                    .filter(|b| *b)
                    .count()
                    > 0;

                ref_updated || params_updated
            }

            AstFieldType::Optional(inner) => inner.pass(type_env, _annotation_env, _scope, errors),
        }
    }
}
