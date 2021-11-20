//! Resolve types to consume and normalize annotations.
//!
//! For example, while in `Type`, the fields carry an optional @column annotation for the
//! column name, here that information is encoded into an attribute of `ResolvedType`.
//! If no @column is provided, the encoded information is set to an appropriate default value.

use std::path::PathBuf;

use anyhow::{anyhow, Result};

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;
use payas_model::model::naming::{ToPlural, ToTableName};
use payas_model::model::GqlTypeModifier;

use crate::ast::ast_types::{AstAnnotationParams, AstArgument, AstFieldType, AstService};
use crate::error::ParserError;
use crate::typechecker::AnnotationMap;
use crate::{
    ast::ast_types::{AstExpr, AstField, AstModel, AstModelKind, FieldSelection},
    typechecker::{PrimitiveType, Type, Typed},
    util::null_span,
};
use serde::{Deserialize, Serialize};

/// Consume typed-checked types and build resolved types
#[derive(Deserialize, Serialize)]
pub struct ResolvedSystem {
    pub types: MappedArena<ResolvedType>,
    pub contexts: MappedArena<ResolvedContext>,
    pub services: MappedArena<ResolvedService>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ResolvedType {
    Primitive(PrimitiveType),
    Composite(ResolvedCompositeType),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedContext {
    pub name: String,
    pub fields: Vec<ResolvedContextField>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedService {
    pub name: String,
    pub module_path: PathBuf,
    pub methods: Vec<ResolvedMethod>,
    pub interceptors: Vec<ResolvedInterceptor>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedMethod {
    pub name: String,
    pub operation_kind: ResolvedMethodType,
    pub is_exported: bool,
    pub access: ResolvedAccess,
    pub arguments: Vec<ResolvedArgument>,
    pub return_type: ResolvedFieldType,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolvedMethodType {
    Query,
    Mutation,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedArgument {
    pub name: String,
    pub typ: ResolvedFieldType,
    pub is_injected: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedInterceptor {
    pub name: String,
    pub arguments: Vec<ResolvedArgument>,
    pub interceptor_kind: ResolvedInterceptorKind,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolvedInterceptorKind {
    Before(AstExpr<Typed>),
    After(AstExpr<Typed>),
    Around(AstExpr<Typed>),
}

impl ResolvedInterceptorKind {
    pub fn expr(&self) -> &AstExpr<Typed> {
        match self {
            ResolvedInterceptorKind::Before(expr) => expr,
            ResolvedInterceptorKind::After(expr) => expr,
            ResolvedInterceptorKind::Around(expr) => expr,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedAccess {
    pub creation: AstExpr<Typed>,
    pub read: AstExpr<Typed>,
    pub update: AstExpr<Typed>,
    pub delete: AstExpr<Typed>,
}

impl ResolvedAccess {
    fn permissive() -> Self {
        ResolvedAccess {
            creation: AstExpr::BooleanLiteral(true, null_span()),
            read: AstExpr::BooleanLiteral(true, null_span()),
            update: AstExpr::BooleanLiteral(true, null_span()),
            delete: AstExpr::BooleanLiteral(true, null_span()),
        }
    }
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolvedCompositeTypeKind {
    Persistent { table_name: String },
    NonPersistent { is_input: bool },
}

impl ToPlural for ResolvedCompositeType {
    fn to_singular(&self) -> String {
        self.name.clone()
    }

    fn to_plural(&self) -> String {
        self.plural_name.clone()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedField {
    pub name: String,
    pub typ: ResolvedFieldType,
    pub kind: ResolvedFieldKind,
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
        match &self.kind {
            ResolvedFieldKind::Persistent {
                is_autoincrement, ..
            } => *is_autoincrement,
            ResolvedFieldKind::NonPersistent => {
                panic!("Tried to get persistence-related information from a non-persistent field!")
            }
        }
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
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolvedFieldKind {
    Persistent {
        column_name: String,
        is_pk: bool,
        is_autoincrement: bool,
        type_hint: Option<ResolvedTypeHint>,
    },
    NonPersistent,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedContextField {
    pub name: String,
    pub typ: ResolvedFieldType,
    pub source: ResolvedContextSource,
}

// For now, ResolvedContextSource and ContextSource have the same structure
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolvedContextSource {
    Jwt { claim: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolvedFieldType {
    Plain(String), // Should really be Id<ResolvedType>, but using String since the former is not serializable as needed by the insta crate
    Optional(Box<ResolvedFieldType>),
    List(Box<ResolvedFieldType>),
}

impl ResolvedFieldType {
    pub fn get_underlying_typename(&self) -> &str {
        match &self {
            ResolvedFieldType::Plain(s) => s,
            ResolvedFieldType::Optional(underlying) => underlying.get_underlying_typename(),
            ResolvedFieldType::List(underlying) => underlying.get_underlying_typename(),
        }
    }

    pub fn get_modifier(&self) -> GqlTypeModifier {
        match &self {
            ResolvedFieldType::Plain(_) => GqlTypeModifier::NonNull,
            ResolvedFieldType::Optional(_) => GqlTypeModifier::Optional,
            ResolvedFieldType::List(_) => GqlTypeModifier::List,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

impl ResolvedFieldType {
    pub fn deref<'a>(&'a self, types: &'a MappedArena<ResolvedType>) -> &'a ResolvedType {
        match self {
            ResolvedFieldType::Plain(name) => types.get_by_key(name).unwrap(),
            ResolvedFieldType::Optional(underlying) | ResolvedFieldType::List(underlying) => {
                underlying.deref(types)
            }
        }
    }
}

pub fn build(types: MappedArena<Type>) -> Result<ResolvedSystem> {
    let mut errors = Vec::new();

    let mut resolved_system = build_shallow(&types, &mut errors)?;
    build_expanded(types, &mut resolved_system, &mut errors);

    if errors.is_empty() {
        Ok(resolved_system)
    } else {
        Err(ParserError::Generic(errors).into())
    }
}

fn build_shallow(
    types: &MappedArena<Type>,
    _errors: &mut Vec<Diagnostic>,
) -> Result<ResolvedSystem> {
    let mut resolved_types: MappedArena<ResolvedType> = MappedArena::default();
    let mut resolved_contexts: MappedArena<ResolvedContext> = MappedArena::default();
    let mut resolved_services: MappedArena<ResolvedService> = MappedArena::default();

    for (_, typ) in types.iter() {
        match typ {
            Type::Primitive(pt) => {
                resolved_types.add(&pt.name(), ResolvedType::Primitive(pt.clone()));
            }
            Type::Composite(ct) if ct.kind == AstModelKind::Persistent => {
                let plural_annotation_value = ct
                    .annotations
                    .get("plural_name")
                    .map(|p| p.as_single().as_string());

                let table_name = ct
                    .annotations
                    .get("table")
                    .map(|p| p.as_single().as_string())
                    .unwrap_or_else(|| ct.name.table_name(plural_annotation_value.clone()));
                let access = build_access(ct.annotations.get("access"));
                resolved_types.add(
                    &ct.name,
                    ResolvedType::Composite(ResolvedCompositeType {
                        name: ct.name.clone(),
                        plural_name: plural_annotation_value.unwrap_or_else(|| ct.name.to_plural()), // fallback to automatically pluralizing name
                        fields: vec![],
                        kind: ResolvedCompositeTypeKind::Persistent { table_name },
                        access,
                    }),
                );
            }
            Type::Composite(ct)
                if ct.kind == AstModelKind::NonPersistent
                    || ct.kind == AstModelKind::NonPersistentInput =>
            {
                let access = build_access(ct.annotations.get("access"));
                resolved_types.add(
                    &ct.name,
                    ResolvedType::Composite(ResolvedCompositeType {
                        name: ct.name.clone(),
                        plural_name: ct
                            .annotations
                            .get("plural_name")
                            .map(|p| p.as_single().as_string())
                            .unwrap_or_else(|| ct.name.to_plural()), // fallback to automatically pluralizing name
                        fields: vec![],
                        kind: ResolvedCompositeTypeKind::NonPersistent {
                            is_input: matches!(ct.kind, AstModelKind::NonPersistentInput),
                        },
                        access,
                    }),
                );
            }
            Type::Composite(ct) if ct.kind == AstModelKind::Context => {
                resolved_contexts.add(
                    &ct.name,
                    ResolvedContext {
                        name: ct.name.clone(),
                        fields: vec![],
                    },
                );
            }
            Type::Service(service) => {
                let module_path = match service.annotations.get("external").unwrap() {
                    AstAnnotationParams::Single(AstExpr::StringLiteral(s, _), _) => s,
                    _ => panic!(),
                }
                .clone();

                let mut full_module_path = service.base_clayfile.clone();
                full_module_path.pop();
                full_module_path.push(module_path);

                // Bundle js/ts files using Deno; we need to bundle even the js files since they may import ts files
                let mut out_path = full_module_path.clone();
                out_path.set_extension("bundle.js");

                let mut child = std::process::Command::new("deno")
                    .args([
                        "bundle",
                        "--no-check",
                        full_module_path.to_str().unwrap(),
                        out_path.to_str().unwrap(),
                    ])
                    .spawn()?;

                child.wait()?;

                // replace import with new path
                full_module_path = out_path;

                fn extract_intercept_annot<'a>(
                    annotations: &'a AnnotationMap,
                    key: &str,
                ) -> Option<&'a AstExpr<Typed>> {
                    annotations.get(key).map(|a| a.as_single())
                }

                resolved_services.add(
                    &service.name,
                    ResolvedService {
                        name: service.name.clone(),
                        module_path: full_module_path,
                        methods: service
                            .methods
                            .iter()
                            .map(|m| {
                                let access = build_access(m.annotations.get("access"));
                                ResolvedMethod {
                                    name: m.name.clone(),
                                    operation_kind: match m.typ.as_str() {
                                        "query" => ResolvedMethodType::Query,
                                        "mutation" => ResolvedMethodType::Mutation,
                                        _ => panic!(),
                                    },
                                    is_exported: m.is_exported,
                                    access,
                                    arguments: vec![],
                                    return_type: ResolvedFieldType::Plain("".to_string()),
                                }
                            })
                            .collect(),
                        interceptors: service
                            .interceptors
                            .iter()
                            .map(|i| {
                                let before_annot = extract_intercept_annot(&i.annotations, "before")
                                    .map(|s| ResolvedInterceptorKind::Before(s.clone()));
                                let after_annot = extract_intercept_annot(&i.annotations, "after")
                                    .map(|s| ResolvedInterceptorKind::After(s.clone()));
                                let around_annot = extract_intercept_annot(&i.annotations, "around")
                                    .map(|s| ResolvedInterceptorKind::Around(s.clone()));

                                let kind_annots = vec![before_annot, after_annot, around_annot];
                                let kind_annots: Vec<_> =
                                    kind_annots.into_iter().flatten().collect();

                                let kind_annot = match kind_annots.as_slice() {
                                    [] => {
                                        panic!("Interceptor must have at least one of the before/after/around annotation")
                                    }
                                    [single] => single,
                                    _ => panic!(
                                        "Interceptor cannot have more than of the before/after/around annotations"
                                    ),
                                };

                                ResolvedInterceptor {
                                    name: i.name.clone(),
                                    arguments: vec![],
                                    interceptor_kind: kind_annot.clone(),
                                }
                            })
                            .collect(),
                    },
                );
            }
            o => panic!(
                "Unable to build shallow type for non-primitve, non-composite type: {:?}",
                o
            ),
        };
    }

    Ok(ResolvedSystem {
        types: resolved_types,
        contexts: resolved_contexts,
        services: resolved_services,
    })
}

fn build_access(access_annotation_params: Option<&AstAnnotationParams<Typed>>) -> ResolvedAccess {
    match access_annotation_params {
        Some(p) => {
            let restrictive = AstExpr::BooleanLiteral(false, null_span());

            // The annotation parameter hierarchy is:
            // value -> query
            //       -> mutation -> create
            //                   -> update
            //                   -> delete
            // Any lower node in the hierarchy get a priority over it parent.

            let (creation, read, update, delete) = match p {
                AstAnnotationParams::Single(default, _) => (default, default, default, default),
                AstAnnotationParams::Map(m, _) => {
                    let query = m.get("query");
                    let mutation = m.get("mutation");
                    let create = m.get("create");
                    let update = m.get("update");
                    let delete = m.get("delete");

                    let default_mutation = mutation.unwrap_or(&restrictive);

                    (
                        create.unwrap_or(default_mutation),
                        query.unwrap_or(&restrictive),
                        update.unwrap_or(default_mutation),
                        delete.unwrap_or(default_mutation),
                    )
                }
                _ => panic!(),
            };

            ResolvedAccess {
                creation: creation.clone(),
                read: read.clone(),
                update: update.clone(),
                delete: delete.clone(),
            }
        }
        None => ResolvedAccess::permissive(),
    }
}

fn build_expanded(
    types: MappedArena<Type>,
    resolved_system: &mut ResolvedSystem,
    errors: &mut Vec<Diagnostic>,
) {
    for (_, typ) in types.iter() {
        if let Type::Composite(ct) = typ {
            if ct.kind == AstModelKind::Persistent
                || ct.kind == AstModelKind::NonPersistent
                || ct.kind == AstModelKind::NonPersistentInput
            {
                build_expanded_persistent_type(ct, &types, resolved_system, errors).unwrap();
            } else if ct.kind == AstModelKind::Context {
                build_expanded_context_type(ct, &types, resolved_system);
            } else {
                todo!()
            }
        } else if let Type::Service(s) = typ {
            build_expanded_service(s, &types, resolved_system);
        }
    }
}

fn build_expanded_service(
    s: &AstService<Typed>,
    types: &MappedArena<Type>,
    resolved_system: &mut ResolvedSystem,
) {
    // build arguments and return type of service methods

    let resolved_services = &mut resolved_system.services;
    let resolved_types = &mut resolved_system.types;

    let existing_type_id = resolved_services.get_id(&s.name).unwrap();
    let existing_service = &resolved_services[existing_type_id];

    let expanded_methods = s
        .methods
        .iter()
        .map(|m| {
            let existing_method = existing_service
                .methods
                .iter()
                .find(|existing_m| m.name == existing_m.name)
                .unwrap();

            ResolvedMethod {
                arguments: m
                    .arguments
                    .iter()
                    .map(|a| resolve_argument(a, types, resolved_types))
                    .collect(),
                return_type: resolve_field_type(
                    &m.return_type.to_typ(types),
                    types,
                    resolved_types,
                ),
                ..existing_method.clone()
            }
        })
        .collect();

    let expanded_interceptors = s
        .interceptors
        .iter()
        .map(|i| {
            let existing_interceptor = existing_service
                .interceptors
                .iter()
                .find(|existing_i| i.name == existing_i.name)
                .unwrap();

            ResolvedInterceptor {
                arguments: i
                    .arguments
                    .iter()
                    .map(|a| resolve_argument(a, types, resolved_types))
                    .collect(),
                ..existing_interceptor.clone()
            }
        })
        .collect();

    let expanded_service = ResolvedService {
        methods: expanded_methods,
        interceptors: expanded_interceptors,
        ..existing_service.clone()
    };

    resolved_services[existing_type_id] = expanded_service;
}

fn build_expanded_persistent_type(
    ct: &AstModel<Typed>,
    types: &MappedArena<Type>,
    resolved_system: &mut ResolvedSystem,
    errors: &mut Vec<Diagnostic>,
) -> Result<()> {
    let resolved_types = &mut resolved_system.types;

    let existing_type_id = resolved_types.get_id(&ct.name).unwrap();
    let existing_type = &resolved_types[existing_type_id];

    if let ResolvedType::Composite(ResolvedCompositeType {
        name,
        plural_name,
        kind,
        access,
        ..
    }) = existing_type
    {
        let resolved_fields = ct
            .fields
            .iter()
            .flat_map(|field| {
                Result::<ResolvedField, anyhow::Error>::Ok(ResolvedField {
                    name: field.name.clone(),
                    typ: resolve_field_type(&field.typ.to_typ(types), types, resolved_types),
                    kind: match kind {
                        ResolvedCompositeTypeKind::Persistent { .. } => {
                            ResolvedFieldKind::Persistent {
                                column_name: compute_column_name(ct, field, types, errors)?,
                                is_pk: field.annotations.contains("pk"),
                                is_autoincrement: field.annotations.contains("autoincrement"),
                                type_hint: build_type_hint(field, types),
                            }
                        }
                        ResolvedCompositeTypeKind::NonPersistent { .. } => {
                            ResolvedFieldKind::NonPersistent
                        }
                    },
                })
            })
            .collect();

        let expanded = ResolvedType::Composite(ResolvedCompositeType {
            name: name.clone(),
            plural_name: plural_name.clone(),
            fields: resolved_fields,
            kind: kind.clone(),
            access: access.clone(),
        });
        resolved_types[existing_type_id] = expanded;
    }
    Ok(())
}

fn build_type_hint(field: &AstField<Typed>, types: &MappedArena<Type>) -> Option<ResolvedTypeHint> {
    ////
    // Part 1: parse out and validate hints for each primitive
    ////

    let size_annotation = field
        .annotations
        .get("size")
        .map(|params| params.as_single().as_number() as usize);

    let bits_annotation = field
        .annotations
        .get("bits")
        .map(|params| params.as_single().as_number() as usize);

    if size_annotation.is_some() && bits_annotation.is_some() {
        panic!("Cannot have both @size and @bits for {}", field.name)
    }

    let int_hint = {
        // TODO: not great that we're 'type checking' here
        // but we need to know the type of the field before constructing the
        // appropriate type hint
        // needed to disambiguate between Int and Float hints
        if field.typ.get_underlying_typename(types).unwrap() != "Int" {
            None
        } else {
            let range_hint = field.annotations.get("range").map(|params| {
                (
                    params.as_map().get("min").unwrap().as_number(),
                    params.as_map().get("max").unwrap().as_number(),
                )
            });

            let bits_hint = if let Some(size) = size_annotation {
                Some(
                    // normalize size into bits
                    if size <= 2 {
                        16
                    } else if size <= 4 {
                        32
                    } else if size <= 8 {
                        64
                    } else {
                        panic!("@size of {} cannot be larger than 8 bytes", field.name)
                    },
                )
            } else if let Some(bits) = bits_annotation {
                if !(bits == 16 || bits == 32 || bits == 64) {
                    panic!("@bits of {} is not 16, 32, or 64", field.name)
                }

                Some(bits)
            } else {
                None
            };

            if bits_hint.is_some() || range_hint.is_some() {
                Some(ResolvedTypeHint::Int {
                    bits: bits_hint,
                    range: range_hint,
                })
            } else {
                // no useful hints to pass along
                None
            }
        }
    };

    let float_hint = {
        // needed to disambiguate between Int and Float hints
        if field.typ.get_underlying_typename(types).unwrap() != "Float" {
            None
        } else {
            let bits_hint = if let Some(size) = size_annotation {
                Some(
                    // normalize size into bits
                    if size <= 4 {
                        24
                    } else if size <= 8 {
                        53
                    } else {
                        panic!("@size of {} cannot be larger than 8 bytes", field.name)
                    },
                )
            } else {
                bits_annotation
            };

            bits_hint.map(|bits| ResolvedTypeHint::Float { bits })
        }
    };

    let number_hint = {
        // needed to disambiguate between DateTime and Decimal hints
        if field.typ.get_underlying_typename(types).unwrap() != "Decimal" {
            None
        } else {
            let precision_hint = field
                .annotations
                .get("precision")
                .map(|p| p.as_single().as_number() as usize);

            let scale_hint = field
                .annotations
                .get("scale")
                .map(|p| p.as_single().as_number() as usize);

            if scale_hint.is_some() && precision_hint.is_none() {
                panic!("@scale is not allowed without specifying @precision")
            }

            // warn the user about possible loss of precision
            if let Some(p) = precision_hint {
                if p > 28 {
                    eprint!("Warning for {}: we currently only support 28 digits of precision for this type! ", field.name);
                    eprint!("You specified {}, values will be rounded: ", p);
                    eprintln!("https://github.com/payalabs/payas/issues/149");
                }
            }

            Some(ResolvedTypeHint::Decimal {
                precision: precision_hint,
                scale: scale_hint,
            })
        }
    };

    let string_hint = {
        let length_annotation = field
            .annotations
            .get("length")
            .map(|p| p.as_single().as_number() as usize);

        // None if there is no length annotation
        length_annotation.map(|length| ResolvedTypeHint::String { length })
    };

    let datetime_hint = {
        // needed to disambiguate between DateTime and Decimal hints
        if field
            .typ
            .get_underlying_typename(types)
            .unwrap()
            .contains("Date")
            || field
                .typ
                .get_underlying_typename(types)
                .unwrap()
                .contains("Time")
            || field.typ.get_underlying_typename(types).unwrap() != "Instant"
        {
            None
        } else {
            field
                .annotations
                .get("precision")
                .map(|p| ResolvedTypeHint::DateTime {
                    precision: p.as_single().as_number() as usize,
                })
        }
    };

    let primitive_hints = vec![
        int_hint,
        float_hint,
        number_hint,
        string_hint,
        datetime_hint,
    ];

    let explicit_dbtype_hint = field
        .annotations
        .get("dbtype")
        .map(|p| p.as_single().as_string())
        .map(|s| ResolvedTypeHint::Explicit {
            dbtype: s.to_uppercase(),
        });

    ////
    // Part 2: make sure user specified a valid combination of hints
    // e.g. they didn't specify hints for two different types
    ////

    let number_of_valid_primitive_hints: usize = primitive_hints
        .iter()
        .map(|hint| if hint.is_some() { 1 } else { 0 })
        .sum();

    let valid_primitive_hints_exist = number_of_valid_primitive_hints > 0;

    if explicit_dbtype_hint.is_some() && valid_primitive_hints_exist {
        panic!(
            "Cannot specify both @dbtype and a primitive specific hint for {}",
            field.name
        )
    }

    if number_of_valid_primitive_hints > 1 {
        panic!("Conflicting type hints specified for {}", field.name)
    }

    ////
    // Part 3: return appropriate hint
    ////

    if explicit_dbtype_hint.is_some() {
        explicit_dbtype_hint
    } else if number_of_valid_primitive_hints == 1 {
        primitive_hints
            .into_iter()
            .find(|hint| hint.is_some())
            .unwrap()
    } else {
        None
    }
}

fn build_expanded_context_type(
    ct: &AstModel<Typed>,
    types: &MappedArena<Type>,
    resolved_system: &mut ResolvedSystem,
) {
    let resolved_contexts = &mut resolved_system.contexts;
    let resolved_types = &mut resolved_system.types;

    let existing_type_id = resolved_contexts.get_id(&ct.name).unwrap();
    let existing_type = &resolved_contexts[existing_type_id];
    let resolved_fields = ct
        .fields
        .iter()
        .map(|field| ResolvedContextField {
            name: field.name.clone(),
            typ: resolve_field_type(&field.typ.to_typ(types), types, resolved_types),
            source: extract_context_source(field),
        })
        .collect();

    let expanded = ResolvedContext {
        name: existing_type.name.clone(),
        fields: resolved_fields,
    };
    resolved_contexts[existing_type_id] = expanded;
}

fn extract_context_source(field: &AstField<Typed>) -> ResolvedContextSource {
    let claim = field
        .annotations
        .get("jwt")
        .map(|p| match p {
            AstAnnotationParams::Single(AstExpr::FieldSelection(selection), _) => match selection {
                FieldSelection::Single(claim, _) => claim.0.clone(),
                _ => panic!("Only simple jwt claim supported"),
            },
            AstAnnotationParams::Single(AstExpr::StringLiteral(name, _), _) => name.clone(),
            AstAnnotationParams::None => field.name.clone(),
            _ => panic!("Expression type other than selection unsupported"),
        })
        .unwrap();

    ResolvedContextSource::Jwt { claim }
}

fn compute_column_name(
    enclosing_type: &AstModel<Typed>,
    field: &AstField<Typed>,
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
) -> Result<String> {
    fn default_column_name(
        enclosing_type: &AstModel<Typed>,
        field: &AstField<Typed>,
        types: &MappedArena<Type>,
        errors: &mut Vec<Diagnostic>,
    ) -> Result<String> {
        match &field.typ {
            AstFieldType::Optional(_) => Ok(field.name.to_string()),
            AstFieldType::Plain(_, _, _, _) => {
                let field_type = field.typ.to_typ(types).deref(types);
                match field_type {
                    Type::Composite(_) => Ok(format!("{}_id", field.name)),
                    Type::Set(typ) => {
                        if let Type::Composite(model) = typ.deref(types) {
                            // OneToMany
                            let matching_fields: Vec<_> = model
                                .fields
                                .into_iter()
                                .filter(|f| f.typ.name() == enclosing_type.name)
                                .collect();

                            match &matching_fields[..] {
                                [] => {
                                    errors.push(
                                    Diagnostic {
                                        level: Level::Error,
                                        message: format!(
                                            "Could not find the matching field of the '{}' type when determining the matching column for '{}'",
                                            enclosing_type.name, field.name
                                        ),
                                        code: Some("C000".to_string()),
                                        spans: vec![SpanLabel {
                                            span: field.span,
                                            style: SpanStyle::Primary,
                                            label: None,
                                        }],
                                    });
                                    Err(anyhow!("Could not find matching field"))
                                }
                                [matching_field] => Ok(format!("{}_id", matching_field.name)),
                                _ => {
                                    errors.push(Diagnostic {
                                        level: Level::Error,
                                        message: format!(
                                            "Found multiple matching fields {} of '{}' type when determining the matching column for '{}'",
                                            matching_fields
                                                .into_iter()
                                                .map(|f| format!("'{}'", f.name))
                                                .collect::<Vec<_>>()
                                                .join(", "), enclosing_type.name, field.name),
                                        code: Some("C000".to_string()),
                                        spans: vec![SpanLabel {
                                            span: field.span,
                                            style: SpanStyle::Primary,
                                            label: None,
                                        }],
                                    });
                                    Err(anyhow!("Could not find matching field"))
                                }
                            }
                        } else {
                            panic!("Sets of non-composites are not supported");
                        }
                    }

                    Type::Array(typ) => {
                        // unwrap type
                        let mut underlying_typ = &typ;
                        while let Type::Array(t) = &**underlying_typ {
                            underlying_typ = t;
                        }

                        if let Type::Primitive(_) = underlying_typ.deref(types) {
                            // base type is a primitive, which means this is an Array
                            Ok(field.name.clone())
                        } else {
                            Err(anyhow!("Arrays of non-primitives are not supported"))
                        }
                    }

                    _ => Ok(field.name.clone()),
                }
            }
        }
    }

    match field
        .annotations
        .get("column")
        .map(|p| p.as_single().as_string())
    {
        Some(name) => Ok(name),
        None => default_column_name(enclosing_type, field, types, errors),
    }
}

fn resolve_field_type(
    typ: &Type,
    types: &MappedArena<Type>,
    resolved_types: &MappedArena<ResolvedType>,
) -> ResolvedFieldType {
    match typ {
        Type::Optional(underlying) => ResolvedFieldType::Optional(Box::new(resolve_field_type(
            underlying.as_ref(),
            types,
            resolved_types,
        ))),
        Type::Reference(id) => {
            ResolvedFieldType::Plain(types[*id].get_underlying_typename(types).unwrap())
        }
        Type::Set(underlying) | Type::Array(underlying) => ResolvedFieldType::List(Box::new(
            resolve_field_type(underlying.as_ref(), types, resolved_types),
        )),
        _ => todo!("Unsupported field type"),
    }
}

fn resolve_argument(
    arg: &AstArgument<Typed>,
    types: &MappedArena<Type>,
    resolved_types: &MappedArena<ResolvedType>,
) -> ResolvedArgument {
    ResolvedArgument {
        name: arg.name.clone(),
        typ: resolve_field_type(&arg.typ.to_typ(types), types, resolved_types),
        is_injected: arg.annotations.get("inject").is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parser, typechecker};
    use std::fs::File;

    #[test]
    fn with_annotations() {
        let src = r#"
        @table("custom_concerts")
        model Concert {
          id: Int @pk @dbtype("bigint") @autoincrement @column("custom_id")
          title: String @column("custom_title") @length(12)
          venue: Venue @column("custom_venue_id")
          reserved: Int @range(min=0, max=300)
          time: Instant @precision(4)
          price: Decimal @precision(10) @scale(2)
        }
        
        @table("venues")
        @plural_name("Venuess")
        model Venue {
          id: Int @pk @autoincrement @column("custom_id")
          name: String @column("custom_name")
          concerts: Set<Concert> @column("custom_venueid")
          capacity: Int @bits(16)
          latitude: Float @size(4)
        }       
        
        @external("bar.js")
        service Foo {
            export query qux(@inject claytip: Claytip, x: Int, y: String): Int
            mutation quuz(): String
        }
        "#;

        File::create("bar.js").unwrap();

        let resolved = create_resolved_system(src);

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn with_defaults() {
        // Note the swapped order between @pk and @autoincrement to assert that our parsing logic permits any order
        let src = r#"
        model Concert {
          id: Int @pk @autoincrement 
          title: String 
          venue: Venue 
          attending: Array<String>
          seating: Array<Array<Boolean>>
        }

        model Venue             {
          id: Int  @autoincrement @pk 
          name:String 
          concerts: Set<Concert> 
        }        
        "#;

        let resolved = create_resolved_system(src);

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn with_optional_fields() {
        let src = r#"
        model Concert {
          id: Int @pk @autoincrement 
          title: String 
          venue: Venue?
          icon: Blob?
        }

        model Venue {
          id: Int @pk @autoincrement
          name: String
          address: String? @column("custom_address")
          concerts: Set<Concert>? @column("custom_venueid")
        }    
        "#;

        let resolved = create_resolved_system(src);

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn with_access() {
        let src = r#"
        context AuthContext {
            role: String @jwt("role")
        }

        @access(AuthContext.role == "ROLE_ADMIN" || self.public)
        model Concert {
          id: Int @pk @autoincrement 
          title: String
          public: Boolean
        }      

        @external("logger.js")
        service Logger {
            @access(AuthContext.role == "ROLE_ADMIN")
            export query log(@inject claytip: Claytip): Boolean
        }
        "#;

        File::create("logger.js").unwrap();

        let resolved = create_resolved_system(src);

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn with_access_default_values() {
        let src = r#"
        context AuthContext {
            role: String @jwt
        }

        @access(AuthContext.role == "ROLE_ADMIN" || self.public)
        model Concert {
          id: Int @pk @autoincrement 
          title: String
          public: Boolean
        }      
        "#;

        let resolved = create_resolved_system(src);

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn field_name_variations() {
        let src = r#"
        model Entity {
          _id: Int @pk @autoincrement
          title_main: String
          title_main1: String
          public1: Boolean
          PUBLIC2: Boolean
          foo123: Int
        }"#;

        let resolved = create_resolved_system(src);

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn column_names_for_non_standard_relational_field_names() {
        let src = r#"
        model Concert {
          id: Int @pk @autoincrement
          title: String
          venuex: Venue // non-standard name
          published: Boolean
        }
        
        model Venue {
          id: Int @pk @autoincrement
          name: String
          concerts: Set<Concert>
          published: Boolean
        }             
        "#;

        let resolved = create_resolved_system(src);

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    fn create_resolved_system(src: &str) -> ResolvedSystem {
        let (parsed, _codemap) = parser::parse_str(src).unwrap();
        let types = typechecker::build(parsed).unwrap();
        build(types).unwrap()
    }
}
