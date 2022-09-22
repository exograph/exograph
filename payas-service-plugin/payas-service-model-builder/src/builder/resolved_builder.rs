use std::io::Write;

use codemap::Span;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_core_model_builder::{
    ast::ast_types::{
        AstAnnotationParams, AstArgument, AstExpr, AstMethodType, AstModelKind, AstService,
    },
    builder::{
        access_builder::{build_access, ResolvedAccess},
        resolved_builder::{
            resolve_field_type, AnnotationMapHelper, ResolvedCompositeType,
            ResolvedCompositeTypeKind, ResolvedField, ResolvedFieldKind, ResolvedFieldType,
            ResolvedType,
        },
    },
    error::ModelBuildingError,
    typechecker::{annotation_map::AnnotationMap, typ::Type, Typed},
};
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::builder::service_skeleton_generator;

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedServiceSystem {
    pub service_types: MappedArena<ResolvedType>,
    pub services: MappedArena<ResolvedService>,
}

pub fn build(types: &MappedArena<Type>) -> Result<ResolvedServiceSystem, ModelBuildingError> {
    let mut errors = Vec::new();

    let resolved_system = resolve(&types, &mut errors)?;

    if errors.is_empty() {
        Ok(resolved_system)
    } else {
        Err(ModelBuildingError::Diagnosis(errors))
    }
}

pub fn resolve(
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
) -> Result<ResolvedServiceSystem, ModelBuildingError> {
    Ok(ResolvedServiceSystem {
        service_types: resolve_service_types(types)?,
        services: resolve_shallow_services(types, errors)?,
    })
}

fn resolve_shallow_services(
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
) -> Result<MappedArena<ResolvedService>, ModelBuildingError> {
    let mut resolved_services: MappedArena<ResolvedService> = MappedArena::default();

    for (_, typ) in types.iter() {
        if let Type::Service(service) = typ {
            resolve_shallow_service(service, types, errors, &mut resolved_services)?;
        }
    }

    Ok(resolved_services)
}

fn resolve_shallow_service(
    service: &AstService<Typed>,
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
    resolved_services: &mut MappedArena<ResolvedService>,
) -> Result<(), ModelBuildingError> {
    let module_path = match service.annotations.get("external").unwrap() {
        AstAnnotationParams::Single(AstExpr::StringLiteral(s, _), _) => s,
        _ => panic!(),
    }
    .clone();

    let mut module_fs_path = service.base_clayfile.clone();
    module_fs_path.pop();
    module_fs_path.push(module_path);
    let extension = module_fs_path.extension().and_then(|e| e.to_str());

    let bundled_script = if extension == Some("ts") || extension == Some("js") {
        service_skeleton_generator::generate_service_skeleton(service, &module_fs_path)?;

        // Bundle js/ts files using Deno; we need to bundle even the js files since they may import ts files
        let bundler_output = std::process::Command::new("deno")
            .args(["bundle", "--no-check", module_fs_path.to_str().unwrap()])
            .output()
            .map_err(|err| {
                ModelBuildingError::Generic(format!(
                    "While trying to invoke `deno` in order to bundle .ts files: {}",
                    err
                ))
            })?;

        if bundler_output.status.success() {
            bundler_output.stdout
        } else {
            std::io::stdout().write_all(&bundler_output.stderr).unwrap();
            return Err(ModelBuildingError::Generic(
                "Deno bundler did not exit successfully".to_string(),
            ));
        }
    } else {
        std::fs::read(&module_fs_path).map_err(|err| {
            ModelBuildingError::Generic(format!(
                "While trying to read bundled service module: {}",
                err
            ))
        })?
    };

    let module_anonymized_path = module_fs_path
        .strip_prefix(&service.base_clayfile.parent().unwrap())
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
                        name: i.name.clone(),
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

fn resolve_service_types(
    types: &MappedArena<Type>,
) -> Result<MappedArena<ResolvedType>, ModelBuildingError> {
    let mut resolved_service_types: MappedArena<ResolvedType> = MappedArena::default();

    for (_, typ) in types.iter() {
        if let Type::Composite(ct) = typ {
            if ct.kind == AstModelKind::NonPersistent || ct.kind == AstModelKind::NonPersistentInput
            {
                let access = build_access(ct.annotations.get("access"));
                let resolved_fields = ct
                    .fields
                    .iter()
                    .map(|field| ResolvedField {
                        name: field.name.clone(),
                        typ: resolve_field_type(&field.typ.to_typ(types), types),
                        kind: ResolvedFieldKind::NonPersistent,
                        default_value: None,
                    })
                    .collect();

                resolved_service_types.add(
                    &ct.name,
                    ResolvedType::Composite(ResolvedCompositeType {
                        name: ct.name.clone(),
                        plural_name: "".to_string(),
                        fields: resolved_fields,
                        kind: ResolvedCompositeTypeKind::NonPersistent {
                            is_input: matches!(ct.kind, AstModelKind::NonPersistentInput),
                        },
                        access,
                    }),
                );
            }
        }
    }

    Ok(resolved_service_types)
}
