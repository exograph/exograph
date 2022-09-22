use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::{mapped_arena::MappedArena, GqlTypeModifier};
use serde::{Deserialize, Serialize};

use crate::{
    ast::ast_types::{AstAnnotation, AstAnnotationParams, AstExpr, AstField, AstModelKind},
    error::ModelBuildingError,
    typechecker::{AnnotationMap, PrimitiveType, Type, Typed},
};

use super::{access_builder::ResolvedAccess, type_builder::ResolvedTypeEnv};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ResolvedType {
    Primitive(PrimitiveType),
    Composite(ResolvedCompositeType),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedCompositeType {
    pub name: String,
    pub plural_name: String,
    pub fields: Vec<ResolvedField>,
    pub kind: ResolvedCompositeTypeKind,
    pub access: ResolvedAccess,
}

impl ResolvedCompositeType {
    pub fn get_table_name(&self) -> &str {
        if let ResolvedCompositeTypeKind::Persistent { table_name } = &self.kind {
            table_name
        } else {
            panic!("Trying to get table name from non-persistent type!")
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ResolvedCompositeTypeKind {
    Persistent { table_name: String },
    NonPersistent { is_input: bool },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolvedFieldType {
    Plain {
        type_name: String, // Should really be Id<ResolvedType>, but using String since the former is not serializable as needed by the insta crate
        is_primitive: bool, // We need to know if the type is primitive, so that we can look into the correct arena in ModelSystem
    },
    Optional(Box<ResolvedFieldType>),
    List(Box<ResolvedFieldType>),
}

impl ResolvedFieldType {
    pub fn get_underlying_typename(&self) -> &str {
        match &self {
            ResolvedFieldType::Plain { type_name, .. } => type_name,
            ResolvedFieldType::Optional(underlying) => underlying.get_underlying_typename(),
            ResolvedFieldType::List(underlying) => underlying.get_underlying_typename(),
        }
    }

    pub fn get_modifier(&self) -> GqlTypeModifier {
        match &self {
            ResolvedFieldType::Plain { .. } => GqlTypeModifier::NonNull,
            ResolvedFieldType::Optional(_) => GqlTypeModifier::Optional,
            ResolvedFieldType::List(_) => GqlTypeModifier::List,
        }
    }

    pub fn is_underlying_type_primitive(&self) -> bool {
        match &self {
            ResolvedFieldType::Plain { is_primitive, .. } => *is_primitive,
            ResolvedFieldType::Optional(underlying) => underlying.is_underlying_type_primitive(),
            ResolvedFieldType::List(underlying) => underlying.is_underlying_type_primitive(),
        }
    }
}

impl ResolvedFieldType {
    pub fn deref<'a>(&'a self, env: &'a ResolvedTypeEnv) -> &'a ResolvedType {
        match self {
            ResolvedFieldType::Plain { type_name, .. } => env.get_by_key(type_name).unwrap(),
            ResolvedFieldType::Optional(underlying) | ResolvedFieldType::List(underlying) => {
                underlying.deref(env)
            }
        }
    }

    pub fn deref_subsystem_type<'a>(
        &'a self,
        types: &'a MappedArena<ResolvedType>,
    ) -> Option<&'a ResolvedType> {
        match self {
            ResolvedFieldType::Plain { type_name, .. } => types.get_by_key(type_name),
            ResolvedFieldType::Optional(underlying) | ResolvedFieldType::List(underlying) => {
                underlying.deref_subsystem_type(types)
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedField {
    pub name: String,
    pub typ: ResolvedFieldType,
    pub kind: ResolvedFieldKind,
    pub default_value: Option<ResolvedFieldDefault>,
}

// TODO: dedup?
impl ResolvedField {
    pub fn get_column_name(&self) -> &str {
        match &self.kind {
            ResolvedFieldKind::Persistent { column_name, .. } => column_name,
            ResolvedFieldKind::NonPersistent => {
                panic!("Tried to get persistence-related information from a non-persistent field!")
            }
        }
    }

    pub fn get_is_pk(&self) -> bool {
        match &self.kind {
            ResolvedFieldKind::Persistent { is_pk, .. } => *is_pk,
            ResolvedFieldKind::NonPersistent => {
                panic!("Tried to get persistence-related information from a non-persistent field!")
            }
        }
    }

    pub fn get_is_autoincrement(&self) -> bool {
        matches!(
            &self.default_value,
            Some(ResolvedFieldDefault::Autoincrement)
        )
    }

    pub fn get_type_hint(&self) -> &Option<ResolvedTypeHint> {
        match &self.kind {
            ResolvedFieldKind::Persistent { type_hint, .. } => type_hint,
            ResolvedFieldKind::NonPersistent => {
                panic!("Tried to get persistence-related information from a non-persistent field!")
            }
        }
    }
}

// what kind of field is this?
// some fields do not need to be persisted, and thus should not carry database-related
// information
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ResolvedFieldKind {
    Persistent {
        column_name: String,
        self_column: bool, // is the column name in the same table or does it point to a column in a different table?
        is_pk: bool,
        type_hint: Option<ResolvedTypeHint>,
        unique_constraints: Vec<String>,
    },
    NonPersistent,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ResolvedTypeHint {
    Explicit {
        dbtype: String,
    },
    Int {
        bits: Option<usize>,
        range: Option<(i64, i64)>,
    },
    Float {
        bits: usize,
    },
    Decimal {
        precision: Option<usize>,
        scale: Option<usize>,
    },
    String {
        length: usize,
    },
    DateTime {
        precision: usize,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolvedFieldDefault {
    Value(Box<AstExpr<Typed>>),
    DatabaseFunction(String),
    Autoincrement,
}

impl ResolvedCompositeType {
    pub fn pk_field(&self) -> Option<&ResolvedField> {
        self.fields.iter().find(|f| {
            if let ResolvedFieldKind::Persistent { is_pk, .. } = f.kind {
                is_pk
            } else {
                false
            }
        })
    }
}

impl ResolvedType {
    pub fn name(&self) -> String {
        match self {
            ResolvedType::Primitive(pt) => pt.name(),
            ResolvedType::Composite(ResolvedCompositeType { name, .. }) => name.to_owned(),
        }
    }

    pub fn plural_name(&self) -> String {
        match self {
            ResolvedType::Primitive(_) => "".to_string(), // unused
            ResolvedType::Composite(ResolvedCompositeType { plural_name, .. }) => {
                plural_name.to_owned()
            }
        }
    }

    pub fn as_primitive(&self) -> PrimitiveType {
        match &self {
            ResolvedType::Primitive(p) => p.clone(),
            _ => panic!("Not a primitive: {:?}", self),
        }
    }

    // useful for relation creation
    pub fn as_composite(&self) -> &ResolvedCompositeType {
        match &self {
            ResolvedType::Composite(c) => c,
            _ => panic!("Cannot get inner composite of type {:?}", self),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedContext {
    pub name: String,
    pub fields: Vec<ResolvedContextField>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedContextField {
    pub name: String,
    pub typ: ResolvedFieldType,
    pub source: ResolvedContextSource,
}

// For now, ResolvedContextSource and ContextSource have the same structure
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContextSource {
    pub annotation: String,
    pub value: Option<String>,
}

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

pub(crate) struct ResolvedBaseSystem {
    pub primitive_types: MappedArena<ResolvedType>,
    pub contexts: MappedArena<ResolvedContext>,
}

pub(crate) fn build(types: &MappedArena<Type>) -> Result<ResolvedBaseSystem, ModelBuildingError> {
    let mut errors = Vec::new();

    let resolved_system = resolve(&types, &mut errors)?;

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
        primitive_types: resolve_primitive_types(types)?,
        contexts: resolve_shallow_contexts(types, errors)?,
    })
}

fn resolve_primitive_types(
    types: &MappedArena<Type>,
) -> Result<MappedArena<ResolvedType>, ModelBuildingError> {
    let mut resolved_primitive_types: MappedArena<ResolvedType> = MappedArena::default();

    for (_, typ) in types.iter() {
        if let Type::Primitive(pt) = typ {
            resolved_primitive_types.add(&pt.name(), ResolvedType::Primitive(pt.clone()));
        }
    }

    Ok(resolved_primitive_types)
}

pub fn resolve_field_type(typ: &Type, types: &MappedArena<Type>) -> ResolvedFieldType {
    match typ {
        Type::Optional(underlying) => {
            ResolvedFieldType::Optional(Box::new(resolve_field_type(underlying.as_ref(), types)))
        }
        Type::Reference(id) => ResolvedFieldType::Plain {
            type_name: types[*id].get_underlying_typename(types).unwrap(),
            is_primitive: matches!(types[*id], Type::Primitive(_)),
        },
        Type::Set(underlying) | Type::Array(underlying) => {
            ResolvedFieldType::List(Box::new(resolve_field_type(underlying.as_ref(), types)))
        }
        _ => todo!("Unsupported field type"),
    }
}

fn resolve_shallow_contexts(
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
                            typ: resolve_field_type(&field.typ.to_typ(types), types),
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
