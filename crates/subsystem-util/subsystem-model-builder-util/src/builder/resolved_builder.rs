// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use codemap::Span;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};

use core_model::primitive_type::InjectedType;
use core_model::types::{FieldType, Named};
use core_model::{mapped_arena::MappedArena, primitive_type::PrimitiveType};
use core_model_builder::ast::ast_types::AstFieldType;
use core_model_builder::builder::resolved_builder::{AnnotationMapHelper, compute_fragment_fields};
use core_model_builder::builder::system_builder::BaseModelSystem;
use core_model_builder::typechecker::AnnotationMap;
use core_model_builder::typechecker::typ::{Module, TypecheckedSystem};
use core_model_builder::{
    ast::ast_types::{
        AstAnnotationParams, AstArgument, AstExpr, AstMethodType, AstModelKind, AstModule,
    },
    error::ModelBuildingError,
    typechecker::{Typed, typ::Type},
};
use serde::{Deserialize, Serialize};

use crate::ScriptProcessor;
use crate::builder::access_builder::build_access;

use super::access_builder::ResolvedAccess;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ResolvedType {
    Primitive(PrimitiveType),
    Injected(InjectedType),
    Composite(ResolvedCompositeType),
}

impl ResolvedType {
    pub fn name(&self) -> String {
        match self {
            ResolvedType::Primitive(pt) => pt.name(),
            ResolvedType::Injected(it) => it.name(),
            ResolvedType::Composite(ResolvedCompositeType { name, .. }) => name.to_owned(),
        }
    }

    pub fn is_primitive(&self) -> bool {
        matches!(self, ResolvedType::Primitive(_))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedCompositeType {
    pub name: String,
    pub fields: Vec<ResolvedField>,
    pub is_input: bool,
    pub access: ResolvedAccess,
    pub doc_comments: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedField {
    pub name: String,
    pub typ: FieldType<ResolvedFieldType>,
    pub default_value: Option<Box<AstExpr<Typed>>>,
    pub doc_comments: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedFieldType {
    pub module_name: Option<String>,
    pub type_name: String,
    pub doc_comments: Option<String>,
}

impl Named for ResolvedFieldType {
    fn name(&self) -> &str {
        &self.type_name
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedModule {
    pub name: String,
    pub script: Vec<u8>,
    pub script_path: String,
    pub methods: Vec<ResolvedMethod>,
    pub interceptors: Vec<ResolvedInterceptor>,
    pub types_defined: HashSet<String>, // Typed defined in the module
    pub doc_comments: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedMethod {
    pub name: String,
    pub operation_kind: ResolvedMethodType,
    pub is_exported: bool,
    pub access: ResolvedAccess,
    pub arguments: Vec<ResolvedArgument>,
    pub return_type: FieldType<ResolvedFieldType>,
    pub doc_comments: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ResolvedMethodType {
    Query,
    Mutation,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedArgument {
    pub name: String,
    pub typ: FieldType<ResolvedFieldType>,
    pub is_injected: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedInterceptor {
    pub module_name: String,
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
pub struct ResolvedModuleSystem {
    pub module_types: MappedArena<ResolvedType>,
    pub modules: MappedArena<ResolvedModule>,
}

pub async fn build(
    typechecked_system: &TypecheckedSystem,
    base_system: &BaseModelSystem,
    module_selection_closure: impl Fn(&AstModule<Typed>) -> Option<String>,
    script_processor: impl ScriptProcessor,
) -> Result<ResolvedModuleSystem, ModelBuildingError> {
    let mut errors = Vec::new();

    let resolved_system = resolve(
        typechecked_system,
        base_system,
        &mut errors,
        module_selection_closure,
        script_processor,
    )
    .await?;

    if errors.is_empty() {
        Ok(resolved_system)
    } else {
        Err(ModelBuildingError::Diagnosis(errors))
    }
}

async fn resolve(
    typechecked_system: &TypecheckedSystem,
    base_system: &BaseModelSystem,
    errors: &mut Vec<Diagnostic>,
    module_selection_closure: impl Fn(&AstModule<Typed>) -> Option<String>,
    script_processor: impl ScriptProcessor,
) -> Result<ResolvedModuleSystem, ModelBuildingError> {
    let resolved_modules = resolve_modules(
        typechecked_system,
        base_system,
        errors,
        module_selection_closure,
        script_processor,
    )
    .await?;

    Ok(ResolvedModuleSystem {
        module_types: resolve_module_types(errors, typechecked_system, |typ| {
            // The type is relevant only if it is a type defined in a relevant module
            // TODO: Improve this by passing only the relevant modules and processing types in the modules
            let type_name = match typ {
                Type::Composite(c) => Some(&c.name),
                _ => None,
            };
            match type_name {
                Some(type_name) => resolved_modules.iter().any(|(_, resolved_module)| {
                    resolved_module
                        .types_defined
                        .iter()
                        .any(|typ| typ == type_name)
                }),
                None => false,
            }
        })?,
        modules: resolved_modules,
    })
}

async fn resolve_modules(
    typechecked_system: &TypecheckedSystem,
    base_system: &BaseModelSystem,
    errors: &mut Vec<Diagnostic>,
    module_selection_closure: impl Fn(&AstModule<Typed>) -> Option<String>,
    script_processor: impl ScriptProcessor,
) -> Result<MappedArena<ResolvedModule>, ModelBuildingError> {
    let mut resolved_modules: MappedArena<ResolvedModule> = MappedArena::default();

    for (_, module) in typechecked_system.modules.iter() {
        let Module(module) = module;

        if let Some(annotation_name) = module_selection_closure(module) {
            resolve_module(
                module,
                base_system,
                typechecked_system,
                annotation_name,
                &typechecked_system.types,
                errors,
                &mut resolved_modules,
                &script_processor,
            )
            .await?;
        }
    }

    Ok(resolved_modules)
}

#[allow(clippy::too_many_arguments)]
async fn resolve_module(
    module: &AstModule<Typed>,
    base_system: &BaseModelSystem,
    typechecked_system: &TypecheckedSystem,
    annotation_name: String,
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
    resolved_modules: &mut MappedArena<ResolvedModule>,
    script_processor: &impl ScriptProcessor,
) -> Result<(), ModelBuildingError> {
    let annotation = module.annotations.get(&annotation_name);

    match annotation {
        // Extract the source path from the annotation
        // `@deno("util/auth.ts")` -> `util/auth.ts`
        Some(AstAnnotationParams::Single(AstExpr::StringLiteral(module_relative_path, _), _)) => {
            // The source path is relative to the module's base exofile
            let mut source_path = module.base_exofile.clone();
            source_path.pop();
            source_path.push(module_relative_path);

            let (script_path, bundled_script) = script_processor
                .process_script(module, base_system, typechecked_system, &source_path)
                .await?;

            fn extract_intercept_annot<'a>(
                annotations: &'a AnnotationMap,
                key: &str,
            ) -> Option<&'a AstExpr<Typed>> {
                annotations.get(key).map(|a| a.as_single())
            }

            resolved_modules.add(
                &module.name,
                ResolvedModule {
                    name: module.name.clone(),
                    script: bundled_script,
                    script_path,
                    methods: module
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
                                return_type: resolve_field_type(&m.return_type.to_typ(types), m.return_type.module_name(), types),
                                doc_comments: m.doc_comments.clone(),
                            }
                        })
                        .collect(),
                    interceptors: module
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
                                module_name: module.name.clone(),
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
                    types_defined: module.types.iter().map(|m| m.name.clone()).collect(),
                    doc_comments: module.doc_comments.clone(),
                },
            );
        }
        // Handle modules without a script path (e.g., @postgres modules that only define types)
        // This includes @postgres, @postgres(schema="..."), etc.
        // We don't include types_defined here because:
        // 1. These types are handled by their own subsystem (postgres), not the Deno subsystem
        // 2. The .d.ts generation for cross-module type references uses AST types directly
        // 3. Including types_defined would cause type resolution to fail when these types
        //    reference other types (like models) that aren't available in this subsystem
        Some(_) => {
            let script_path = module.base_exofile.clone();
            script_processor
                .process_script(module, base_system, typechecked_system, &script_path)
                .await?;

            resolved_modules.add(
                &module.name,
                ResolvedModule {
                    name: module.name.clone(),
                    script: vec![],
                    script_path: "".to_string(),
                    methods: vec![],
                    interceptors: vec![],
                    types_defined: HashSet::new(), // Empty - types are for .d.ts generation only, not resolution
                    doc_comments: module.doc_comments.clone(),
                },
            );
        }
        None => panic!("Module was selected but annotation not found"),
    }

    Ok(())
}

fn resolve_argument(arg: &AstArgument<Typed>, types: &MappedArena<Type>) -> ResolvedArgument {
    ResolvedArgument {
        name: arg.name.clone(),
        typ: resolve_field_type(&arg.typ.to_typ(types), arg.typ.module_name(), types),
        is_injected: arg.annotations.get("inject").is_some(),
    }
}

fn resolve_module_input_types(
    errors: &mut Vec<Diagnostic>,
    resolved_module_types: &MappedArena<ResolvedType>,
    typechecked_system: &TypecheckedSystem,
) -> Result<Vec<String>, ModelBuildingError> {
    // 1. collect types used as arguments (input) and return types (output)
    type IsInput = bool;
    let mut types_used: Vec<(String, Span, IsInput)> = vec![];

    fn get_underlying_type_name(field_type: &AstFieldType<Typed>) -> String {
        match field_type {
            AstFieldType::Plain(_, name, type_params, ..) => match name.as_str() {
                "Set" | "Array" => get_underlying_type_name(&type_params[0]),
                _ => name.clone(),
            },
            AstFieldType::Optional(underlying) => get_underlying_type_name(underlying),
        }
    }

    for (_, Module(module)) in typechecked_system.modules.iter() {
        for method in module.methods.iter() {
            for argument in method.arguments.iter() {
                types_used.push((
                    get_underlying_type_name(&argument.typ),
                    argument.typ.span(),
                    true,
                ))
            }

            types_used.push((
                get_underlying_type_name(&method.return_type),
                method.return_type.span(),
                false,
            ))
        }

        for interceptor in module.interceptors.iter() {
            for argument in interceptor.arguments.iter() {
                types_used.push((
                    get_underlying_type_name(&argument.typ),
                    argument.typ.span(),
                    true,
                ))
            }
        }
    }

    // 2. filter out primitives
    let types_used = types_used.iter().filter(|(type_name, ..)| {
        if let Some(typ) = resolved_module_types.get_by_key(type_name) {
            !typ.is_primitive()
        } else {
            true
        }
    });

    let (input_types, output_types): (Vec<_>, Vec<_>) =
        types_used.clone().partition(|(_, _, is_input)| *is_input);

    // 3. check types
    for (typ_name, span, is_input) in types_used {
        let (opposite_descriptor, opposite_types) = if *is_input {
            ("an output type", &output_types)
        } else {
            ("an input type", &input_types)
        };

        // check type against opposite list
        if let Some(opposite_type) = opposite_types
            .iter()
            .find(|(opposite_typ_name, ..)| opposite_typ_name == typ_name)
        {
            // FIXME: add a resolved builder snapshot unit test case for this error
            errors.push(
                Diagnostic {
                level: Level::Error,
                message:
                    format!("Type {} was used as {} somewhere else in the model. Types may only be used as either an input type or an output type.", 
                            typ_name, opposite_descriptor),
                code: Some("C000".to_string()),
                spans: vec![
                    SpanLabel {
                        span: *span,
                        style: SpanStyle::Primary,
                        label: Some("conflicting usage".to_owned()),
                    },
                    SpanLabel {
                        span: opposite_type.1,
                        style: SpanStyle::Secondary,
                        label: Some(opposite_descriptor.to_string()),
                    },
                ],
            });

            return Err(ModelBuildingError::Diagnosis(errors.clone()));
        }
    }

    let mut input_type_names = vec![];
    for (input_type_name, _, _) in input_types.iter() {
        if !input_type_names.contains(input_type_name) {
            input_type_names.push(input_type_name.clone());
        }
    }

    Ok(input_type_names)
}

fn resolve_module_types(
    errors: &mut Vec<Diagnostic>,
    typechecked_system: &TypecheckedSystem,
    is_relevant: impl Fn(&Type) -> bool,
) -> Result<MappedArena<ResolvedType>, ModelBuildingError> {
    let mut resolved_module_types: MappedArena<ResolvedType> = MappedArena::default();

    for (_, typ) in typechecked_system.types.iter() {
        if let Type::Primitive(pt) = typ {
            // Adopt the primitive types as a ModuleType
            resolved_module_types.add(&pt.name(), ResolvedType::Primitive(pt.clone()));
        }
        if let Type::Injected(it) = typ {
            // Adopt the injected types as a ModuleType
            resolved_module_types.add(&it.name(), ResolvedType::Injected(it.clone()));
        }
    }

    let input_types =
        resolve_module_input_types(errors, &resolved_module_types, typechecked_system)?;

    for (_, typ) in typechecked_system.types.iter() {
        match typ {
            // Adopt the primitive types as a ModuleType
            Type::Primitive(pt) => {
                resolved_module_types.add(&pt.name(), ResolvedType::Primitive(pt.clone()));
            }
            Type::Composite(ct) if is_relevant(typ) => {
                if ct.kind == AstModelKind::Type {
                    let access = build_access(ct.annotations.get("access"));

                    let fragment_fields = compute_fragment_fields(ct, errors, typechecked_system);

                    let resolved_fields = ct
                        .fields
                        .iter()
                        .chain(fragment_fields.iter().cloned())
                        .map(|field| ResolvedField {
                            name: field.name.clone(),
                            typ: resolve_field_type(
                                &field.typ.to_typ(&typechecked_system.types),
                                field.typ.module_name(),
                                &typechecked_system.types,
                            ),
                            default_value: None,
                            doc_comments: field.doc_comments.clone(),
                        })
                        .collect();

                    resolved_module_types.add(
                        &ct.name,
                        ResolvedType::Composite(ResolvedCompositeType {
                            name: ct.name.clone(),
                            fields: resolved_fields,
                            is_input: input_types.contains(&ct.name),
                            access,
                            doc_comments: ct.doc_comments.clone(),
                        }),
                    );
                }
            }
            _ => {}
        }
    }

    Ok(resolved_module_types)
}

fn resolve_field_type(
    typ: &Type,
    module_name: Option<String>,
    types: &MappedArena<Type>,
) -> FieldType<ResolvedFieldType> {
    match typ {
        Type::Optional(underlying) => FieldType::Optional(Box::new(resolve_field_type(
            underlying.as_ref(),
            module_name,
            types,
        ))),
        Type::Reference(id) => {
            let tpe = &types[*id];
            let doc_comments = match tpe {
                Type::Composite(ct) => ct.doc_comments.clone(),
                _ => None,
            };
            FieldType::Plain(ResolvedFieldType {
                type_name: tpe.get_underlying_typename(types).unwrap(),
                module_name,
                doc_comments,
            })
        }
        Type::Set(underlying) | Type::Array(underlying) => FieldType::List(Box::new(
            resolve_field_type(underlying.as_ref(), module_name, types),
        )),
        _ => {
            panic!("Unsupported field type")
        }
    }
}

#[cfg(not(target_family = "wasm"))]
#[cfg(test)]
mod tests {
    use std::path::Path;

    use async_trait::async_trait;
    use builder::{
        load_subsystem_builders, parser,
        typechecker::{self, annotation_map::AnnotationMapImpl},
    };
    use codemap::CodeMap;
    use core_model_builder::{
        ast::ast_types::AstModule,
        builder::system_builder::BaseModelSystem,
        error::ModelBuildingError,
        typechecker::{Typed, typ::TypecheckedSystem},
    };

    use crate::ScriptProcessor;

    use super::{ResolvedModuleSystem, build};

    #[tokio::test]
    async fn type_disambiguation() {
        let model = r#"
            @deno("x.ts")
            module TestModule {
                type Bar {
                    a: String
                    b: Boolean
                    c: Int
                }

                type FooInput {
                    a: String
                    b: Boolean
                }

                mutation setFoo(key: Int, value: FooInput): Boolean
                query getBar(key: Int): Bar
            } 
        "#;

        assert_success(model).await;
    }

    #[tokio::test]
    async fn input_type_used_as_output_type() {
        let model = r#"
            @deno("x.ts")
            module TestModule {
                type FooInput {
                    a: String
                    b: Boolean
                }

                mutation setFoo(key: Int, value: FooInput): Boolean
                query getFoo(key: Int): FooInput
            } 
        "#;

        assert_err(model).await;
    }

    async fn assert_success(src: &str) {
        assert!(create_resolved_system(src).await.is_ok())
    }

    async fn assert_err(src: &str) {
        assert!(create_resolved_system(src).await.is_err())
    }

    struct TestScriptProcessor {}

    #[async_trait]
    impl ScriptProcessor for TestScriptProcessor {
        async fn process_script(
            &self,
            _module: &AstModule<Typed>,
            _base_system: &BaseModelSystem,
            _typechecked_system: &TypecheckedSystem,
            path: &Path,
        ) -> Result<(String, Vec<u8>), ModelBuildingError> {
            Ok((path.to_str().unwrap().to_string(), vec![]))
        }
    }

    async fn create_resolved_system(src: &str) -> Result<ResolvedModuleSystem, ModelBuildingError> {
        let mut codemap = CodeMap::new();
        let subsystem_builders =
            load_subsystem_builders(vec![
                Box::new(deno_builder::DenoSubsystemBuilder::default()),
            ])
            .unwrap();
        let parsed = parser::parse_str(src, &mut codemap, "input.exo")
            .map_err(|e| ModelBuildingError::Generic(format!("{e:?}")))?;
        let types = typechecker::build(&subsystem_builders, parsed)
            .map_err(|e| ModelBuildingError::Generic(format!("{e:?}")))?;
        let base_system = core_model_builder::builder::system_builder::build(&types)?;

        build(
            &types,
            &base_system,
            |module| module.annotations.get("deno").map(|_| "deno".to_string()),
            TestScriptProcessor {},
        )
        .await
    }
}
