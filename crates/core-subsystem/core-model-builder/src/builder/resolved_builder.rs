// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::{mapped_arena::MappedArena, primitive_type::PrimitiveType, types::FieldType};
use serde::{Deserialize, Serialize};

use crate::{
    ast::ast_types::{AstAnnotation, AstAnnotationParams, AstExpr, AstField, AstModelKind},
    error::ModelBuildingError,
    typechecker::{AnnotationMap, Type, Typed},
};

pub trait AnnotationMapHelper {
    fn get<'a>(&'a self, field_name: &str) -> Option<&'a AstAnnotationParams<Typed>>;

    fn contains(&self, field_name: &str) -> bool {
        self.get(field_name).is_some()
    }

    fn iter(&self) -> std::collections::hash_map::Iter<'_, String, AstAnnotation<Typed>>;
}

impl AnnotationMapHelper for AnnotationMap {
    fn get<'a>(&'a self, field_name: &str) -> Option<&'a AstAnnotationParams<Typed>> {
        self.annotations.get(field_name).map(|a| &a.params)
    }

    fn iter(&self) -> std::collections::hash_map::Iter<'_, String, AstAnnotation<Typed>> {
        self.annotations.iter()
    }
}

pub trait AstAnnotationHelper {
    fn as_single(&self) -> String;
}

impl AstAnnotationHelper for AstAnnotation<Typed> {
    fn as_single(&self) -> String {
        self.params.as_single().as_string()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedContext {
    pub name: String,
    pub fields: Vec<ResolvedContextField>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedContextField {
    pub name: String,
    pub typ: ResolvedContextFieldType,
    pub source: ResolvedContextSource,
}

pub type ResolvedContextFieldType = FieldType<PrimitiveType>;

// For now, ResolvedContextSource and ContextSource have the same structure
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContextSource {
    pub annotation: String,
    pub value: Option<String>,
}

pub(crate) struct ResolvedBaseSystem {
    pub contexts: MappedArena<ResolvedContext>,
}

pub(crate) fn build(types: &MappedArena<Type>) -> Result<ResolvedBaseSystem, ModelBuildingError> {
    let mut errors = Vec::new();

    let resolved_system = resolve(types, &mut errors)?;

    if errors.is_empty() {
        Ok(resolved_system)
    } else {
        Err(ModelBuildingError::Diagnosis(errors))
    }
}

fn resolve(
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
) -> Result<ResolvedBaseSystem, ModelBuildingError> {
    Ok(ResolvedBaseSystem {
        contexts: resolve_contexts(types, errors)?,
    })
}

fn resolve_contexts(
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
) -> Result<MappedArena<ResolvedContext>, ModelBuildingError> {
    let mut resolved_contexts: MappedArena<ResolvedContext> = MappedArena::default();

    for (_, typ) in types.iter() {
        if let Type::Composite(ct) = typ {
            if ct.kind == AstModelKind::Context {
                let resolved_fields = ct
                    .fields
                    .iter()
                    .flat_map(|field| {
                        Some(ResolvedContextField {
                            name: field.name.clone(),
                            typ: resolve_context_field_type(&field.typ.to_typ(types), types),
                            source: extract_context_source(field, errors)?,
                        })
                    })
                    .collect();

                resolved_contexts.add(
                    &ct.name,
                    ResolvedContext {
                        name: ct.name.clone(),
                        fields: resolved_fields,
                    },
                );
            }
        }
    }

    Ok(resolved_contexts)
}

fn resolve_context_field_type(typ: &Type, types: &MappedArena<Type>) -> ResolvedContextFieldType {
    match typ.deref(types) {
        Type::Primitive(pt) => ResolvedContextFieldType::Plain(pt),
        Type::Optional(underlying) => ResolvedContextFieldType::Optional(Box::new(
            resolve_context_field_type(&underlying, types),
        )),
        Type::Set(underlying) | Type::Array(underlying) => {
            ResolvedContextFieldType::List(Box::new(resolve_context_field_type(&underlying, types)))
        }
        _ => panic!("Unexpected type in context field {}", typ.deref(types)),
    }
}

fn extract_context_source(
    field: &AstField<Typed>,
    errors: &mut Vec<Diagnostic>,
) -> Option<ResolvedContextSource> {
    // to determine the source for this context field, extract a single annotation from it
    //
    // context source annotations are not resolved fully here in ResolvedBuilder
    // instead we extract the annotation name and value here and resolve it dynamically later
    match field.annotations.iter().len() {
        0 => {
            // found no annotations! contexts need at least one
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!("No source for context field `{}`", field.name),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: field.span,
                    style: SpanStyle::Primary,
                    label: None,
                }],
            });
            None
        }
        1 => {
            // found exactly one annotation
            // extract it
            let annotation = field.annotations.iter().last().unwrap().1;

            // extract the value from the annotation
            let value = match &annotation.params {
                AstAnnotationParams::Single(AstExpr::StringLiteral(string, _), _) => {
                    Some(string.clone())
                }

                AstAnnotationParams::None => None,

                _ => panic!(
                    "Annotation parameters other than single literal and none unsupported for @{}",
                    annotation.name
                ),
            };

            // return context source
            Some(ResolvedContextSource {
                annotation: annotation.name.clone(),
                value,
            })
        }
        _ => {
            // found more than one annotation! we cannot populate a context field from two sources
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "Cannot have more than one source for context field `{}`",
                    field.name
                ),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: field.span,
                    style: SpanStyle::Primary,
                    label: None,
                }],
            });
            None
        }
    }
}
