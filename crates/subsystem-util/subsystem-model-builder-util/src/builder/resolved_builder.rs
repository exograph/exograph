use std::path::PathBuf;

use codemap::Span;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};

use core_model::{mapped_arena::MappedArena, primitive_type::PrimitiveType};
use core_model_builder::ast::ast_types::AstFieldType;
use core_model_builder::builder::resolved_builder::AnnotationMapHelper;
use core_model_builder::typechecker::AnnotationMap;
use core_model_builder::{
    ast::ast_types::{
        AstAnnotationParams, AstArgument, AstExpr, AstMethodType, AstModelKind, AstService,
    },
    error::ModelBuildingError,
    typechecker::{typ::Type, Typed},
};
use serde::{Deserialize, Serialize};
use subsystem_model_util::types::ServiceTypeModifier;

use crate::builder::access_builder::build_access;

use super::access_builder::ResolvedAccess;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ResolvedType {
    Primitive(PrimitiveType),
    Composite(ResolvedCompositeType),
}

impl ResolvedType {
    pub fn name(&self) -> String {
        match self {
            ResolvedType::Primitive(pt) => pt.name(),
            ResolvedType::Composite(ResolvedCompositeType { name, .. }) => name.to_owned(),
        }
    }

    pub fn is_primitive(&self) -> bool {
        matches!(self, ResolvedType::Primitive(_))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedCompositeType {
    pub name: String,
    pub fields: Vec<ResolvedField>,
    pub is_input: bool,
    pub access: ResolvedAccess,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedField {
    pub name: String,
    pub typ: ResolvedFieldType,
    pub default_value: Option<Box<AstExpr<Typed>>>,
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

    pub fn get_modifier(&self) -> ServiceTypeModifier {
        match &self {
            ResolvedFieldType::Plain { .. } => ServiceTypeModifier::NonNull,
            ResolvedFieldType::Optional(_) => ServiceTypeModifier::Optional,
            ResolvedFieldType::List(_) => ServiceTypeModifier::List,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedService {
    pub name: String,
    pub script: Vec<u8>,
    pub script_path: String,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
    pub service_name: String,
    pub method_name: String,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedServiceSystem {
    pub service_types: MappedArena<ResolvedType>,
    pub services: MappedArena<ResolvedService>,
}

pub fn build(
    types: &MappedArena<Type>,
    service_selection_predicate: impl Fn(&AstService<Typed>) -> bool,
    process_script: impl Fn(&AstService<Typed>, &PathBuf) -> Result<Vec<u8>, ModelBuildingError>,
) -> Result<ResolvedServiceSystem, ModelBuildingError> {
    let mut errors = Vec::new();

    let resolved_system = resolve(
        types,
        &mut errors,
        service_selection_predicate,
        process_script,
    )?;

    if errors.is_empty() {
        Ok(resolved_system)
    } else {
        Err(ModelBuildingError::Diagnosis(errors))
    }
}

fn resolve(
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
    service_selection_predicate: impl Fn(&AstService<Typed>) -> bool,
    process_script: impl Fn(&AstService<Typed>, &PathBuf) -> Result<Vec<u8>, ModelBuildingError>,
) -> Result<ResolvedServiceSystem, ModelBuildingError> {
    Ok(ResolvedServiceSystem {
        service_types: resolve_service_types(errors, types)?,
        services: resolve_services(types, errors, service_selection_predicate, &process_script)?,
    })
}

fn resolve_services(
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
    service_selection_predicate: impl Fn(&AstService<Typed>) -> bool,
    process_script: impl Fn(&AstService<Typed>, &PathBuf) -> Result<Vec<u8>, ModelBuildingError>,
) -> Result<MappedArena<ResolvedService>, ModelBuildingError> {
    let mut resolved_services: MappedArena<ResolvedService> = MappedArena::default();

    for (_, typ) in types.iter() {
        if let Type::Service(service) = typ {
            if service_selection_predicate(service) {
                resolve_service(
                    service,
                    types,
                    errors,
                    &mut resolved_services,
                    &process_script,
                )?;
            }
        }
    }

    Ok(resolved_services)
}

fn resolve_service(
    service: &AstService<Typed>,
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
    resolved_services: &mut MappedArena<ResolvedService>,
    process_script: &impl Fn(&AstService<Typed>, &PathBuf) -> Result<Vec<u8>, ModelBuildingError>,
) -> Result<(), ModelBuildingError> {
    let module_path = match service.annotations.get("external").unwrap() {
        AstAnnotationParams::Single(AstExpr::StringLiteral(s, _), _) => s,
        _ => panic!(),
    }
    .clone();

    let mut module_fs_path = service.base_clayfile.clone();
    module_fs_path.pop();
    module_fs_path.push(module_path);

    let bundled_script = process_script(service, &module_fs_path)?;

    let module_anonymized_path = module_fs_path
        .strip_prefix(service.base_clayfile.parent().unwrap())
        .unwrap();

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
            script: bundled_script,
            script_path: module_anonymized_path.to_str().expect("Script path was not UTF-8").to_string(),
            methods: service
                .methods
                .iter()
                .map(|m| {
                    let access = build_access(m.annotations.get("access"));
                    ResolvedMethod {
                        name: m.name.clone(),
                        operation_kind: match m.typ {
                            AstMethodType::Query   => ResolvedMethodType::Query,
                            AstMethodType::Mutation => ResolvedMethodType::Mutation,
                        },
                        is_exported: m.is_exported,
                        access,
                        arguments: m
                            .arguments
                            .iter()
                            .map(|a| resolve_argument(a, types))
                            .collect(),
                        return_type: resolve_field_type(&m.return_type.to_typ(types), types),
                    }
                })
                .collect(),
            interceptors: service
                .interceptors
                .iter()
                .flat_map(|i| {
                    let before_annot = extract_intercept_annot(&i.annotations, "before")
                        .map(|s| ResolvedInterceptorKind::Before(s.clone()));
                    let after_annot = extract_intercept_annot(&i.annotations, "after")
                        .map(|s| ResolvedInterceptorKind::After(s.clone()));
                    let around_annot = extract_intercept_annot(&i.annotations, "around")
                        .map(|s| ResolvedInterceptorKind::Around(s.clone()));

                    let kind_annots = vec![before_annot, after_annot, around_annot];
                    let kind_annots: Vec<_> =
                        kind_annots.into_iter().flatten().collect();

                    fn create_diagnostic<T>(message: &str, span: Span, errors: &mut Vec<Diagnostic>,) -> Result<T, ModelBuildingError> {
                        errors.push(
                            Diagnostic {
                                level: Level::Error,
                                message: message.to_string(),
                                code: Some("C000".to_string()),
                                spans: vec![SpanLabel {
                                    span,
                                    style: SpanStyle::Primary,
                                    label: None,
                                }],
                            });
                        Err(ModelBuildingError::Diagnosis(errors.clone()))
                    }

                    let kind_annot = match kind_annots.as_slice() {
                        [] => {
                            create_diagnostic("Interceptor must have at least one of the before/after/around annotation", i.span, errors)
                        }
                        [single] => Ok(single),
                        _ => create_diagnostic(
                            "Interceptor cannot have more than of the before/after/around annotations", i.span, errors
                        ),
                    }?;

                    Result::<ResolvedInterceptor, ModelBuildingError>::Ok(ResolvedInterceptor {
                        service_name: service.name.clone(),
                        method_name: i.name.clone(),
                        arguments: i
                            .arguments
                            .iter()
                            .map(|a| resolve_argument(a, types))
                            .collect(),
                        interceptor_kind: kind_annot.clone(),
                    })
                })
                .collect(),
        },
    );

    Ok(())
}

fn resolve_argument(arg: &AstArgument<Typed>, types: &MappedArena<Type>) -> ResolvedArgument {
    ResolvedArgument {
        name: arg.name.clone(),
        typ: resolve_field_type(&arg.typ.to_typ(types), types),
        is_injected: arg.annotations.get("inject").is_some(),
    }
}

fn resolve_service_input_types(
    errors: &mut Vec<Diagnostic>,
    resolved_service_types: &MappedArena<ResolvedType>,
    types: &MappedArena<Type>,
) -> Result<Vec<String>, ModelBuildingError> {
    // 1. collect types used as arguments (input) and return types (output)
    type IsInput = bool;
    let mut types_used: Vec<(AstFieldType<Typed>, IsInput)> = vec![];

    for (_, typ) in types.iter() {
        if let Type::Service(service) = typ {
            for method in service.methods.iter() {
                for argument in method.arguments.iter() {
                    types_used.push((argument.typ.clone(), true))
                }

                types_used.push((method.return_type.clone(), false))
            }

            for interceptor in service.interceptors.iter() {
                for argument in interceptor.arguments.iter() {
                    types_used.push((argument.typ.clone(), true))
                }
            }
        }
    }

    // 2. filter out primitives
    let types_used = types_used.iter().filter(|(arg, _)| {
        if let Some(typ) = resolved_service_types.get_by_key(&arg.name()) {
            !typ.is_primitive()
        } else {
            true
        }
    });

    let (input_types, output_types): (Vec<_>, Vec<_>) =
        types_used.clone().partition(|(_, is_input)| *is_input);

    // 3. check types
    for (typ, is_input) in types_used {
        let (opposite_descriptor, opposite_types) = if *is_input {
            ("an output type", &output_types)
        } else {
            ("an input type", &input_types)
        };

        // check type against opposite list
        if let Some(opposite_type) = opposite_types
            .iter()
            .find(|(opposite_typ, _)| opposite_typ.name() == typ.name())
        {
            // FIXME: add a resolved builder snapshot unit test case for this error
            errors.push(
                Diagnostic {
                level: Level::Error,
                message: format!("Type {} was used as {} somewhere else in the model. Types may only be used as either an input type or an output type.", typ.name(), opposite_descriptor),
                code: Some("C000".to_string()),
                spans: vec![
                    SpanLabel {
                        span: typ.span(),
                        style: SpanStyle::Primary,
                        label: Some("conflicting usage".to_owned()),
                    },
                    SpanLabel {
                        span: opposite_type.0.span(),
                        style: SpanStyle::Secondary,
                        label: Some(opposite_descriptor.to_string()),
                    },
                ],
            });

            return Err(ModelBuildingError::Diagnosis(errors.clone()));
        }
    }

    let mut input_type_names = vec![];
    for (input_type, _) in input_types.iter() {
        if !input_type_names.contains(&input_type.name()) {
            input_type_names.push(input_type.name())
        }
    }

    Ok(input_type_names)
}

fn resolve_service_types(
    errors: &mut Vec<Diagnostic>,
    types: &MappedArena<Type>,
) -> Result<MappedArena<ResolvedType>, ModelBuildingError> {
    let mut resolved_service_types: MappedArena<ResolvedType> = MappedArena::default();

    for (_, typ) in types.iter() {
        if let Type::Primitive(pt) = typ {
            // Adopt the primitive types as a ServiceType
            resolved_service_types.add(&pt.name(), ResolvedType::Primitive(pt.clone()));
        }
    }

    let input_types = resolve_service_input_types(errors, &resolved_service_types, types)?;

    for (_, typ) in types.iter() {
        match typ {
            // Adopt the primitive types as a ServiceType
            Type::Primitive(pt) => {
                resolved_service_types.add(&pt.name(), ResolvedType::Primitive(pt.clone()));
            }
            Type::Composite(ct) => {
                if ct.kind == AstModelKind::NonPersistent {
                    let access = build_access(ct.annotations.get("access"));
                    let resolved_fields = ct
                        .fields
                        .iter()
                        .map(|field| ResolvedField {
                            name: field.name.clone(),
                            typ: resolve_field_type(&field.typ.to_typ(types), types),
                            default_value: None,
                        })
                        .collect();

                    resolved_service_types.add(
                        &ct.name,
                        ResolvedType::Composite(ResolvedCompositeType {
                            name: ct.name.clone(),
                            fields: resolved_fields,
                            is_input: input_types.contains(&ct.name),
                            access,
                        }),
                    );
                }
            }
            _ => {}
        }
    }

    Ok(resolved_service_types)
}

fn resolve_field_type(typ: &Type, types: &MappedArena<Type>) -> ResolvedFieldType {
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
