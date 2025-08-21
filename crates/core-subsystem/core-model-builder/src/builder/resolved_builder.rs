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
    ast::ast_types::{
        AstAnnotation, AstAnnotationParams, AstExpr, AstField, AstModel, AstModelKind,
    },
    error::ModelBuildingError,
    typechecker::{AnnotationMap, Type, Typed, typ::TypecheckedSystem},
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
    pub doc_comments: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedContextField {
    pub name: String,
    pub typ: ResolvedContextFieldType,
    pub source: ResolvedContextSource,
    pub doc_comments: Option<String>,
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
        if let Type::Composite(ct) = typ
            && ct.kind == AstModelKind::Context
        {
            let resolved_fields: Vec<_> = ct
                .fields
                .iter()
                .flat_map(|field| {
                    let typ = match resolve_context_field_type(&field.typ.to_typ(types), types) {
                        Ok(typ) => Some(typ),
                        Err(e) => {
                            errors.push(Diagnostic {
                                level: Level::Error,
                                message: e.to_string(),
                                code: Some("C000".to_string()),
                                spans: vec![SpanLabel {
                                    span: field.span,
                                    style: SpanStyle::Primary,
                                    label: None,
                                }],
                            });
                            None
                        }
                    };

                    typ.and_then(|typ| {
                        extract_context_source(field, errors).map(|source| ResolvedContextField {
                            name: field.name.clone(),
                            typ,
                            source,
                            doc_comments: field.doc_comments.clone(),
                        })
                    })
                })
                .collect();

            resolved_contexts.add(
                &ct.name,
                ResolvedContext {
                    name: ct.name.clone(),
                    fields: resolved_fields,
                    doc_comments: ct.doc_comments.clone(),
                },
            );
        }
    }

    Ok(resolved_contexts)
}

fn resolve_context_field_type(
    typ: &Type,
    types: &MappedArena<Type>,
) -> Result<ResolvedContextFieldType, ModelBuildingError> {
    match typ.deref(types) {
        Type::Primitive(pt) => Ok(ResolvedContextFieldType::Plain(pt)),
        Type::Optional(underlying) => Ok(ResolvedContextFieldType::Optional(Box::new(
            resolve_context_field_type(&underlying, types)?,
        ))),
        Type::Set(underlying) | Type::Array(underlying) => Ok(ResolvedContextFieldType::List(
            Box::new(resolve_context_field_type(&underlying, types)?),
        )),
        _ => Err(ModelBuildingError::Generic(
            "Unexpected type in context field".to_string(),
        )),
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

pub fn compute_fragment_fields<'a>(
    ct: &'a AstModel<Typed>,
    errors: &mut Vec<Diagnostic>,
    typechecked_system: &'a TypecheckedSystem,
) -> Vec<&'a AstField<Typed>> {
    ct.fragment_references
        .iter()
        .flat_map(|fragment_reference| {
            let fragment_type = typechecked_system
                .types
                .get_by_key(&fragment_reference.name);
            match &fragment_type {
                Some(Type::Composite(ft)) => ft.fields.iter().collect(),
                Some(_) => {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "Fragment type {} is not a composite type",
                            fragment_reference.name
                        ),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: fragment_reference.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    });
                    vec![]
                }
                None => {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: format!("Fragment type {} not found", fragment_reference.name),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: fragment_reference.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    });
                    vec![]
                }
            }
        })
        .collect::<Vec<_>>()
}
