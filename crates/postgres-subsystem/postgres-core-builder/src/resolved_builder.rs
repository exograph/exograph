// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Resolve types to consume and normalize annotations.
//!
//! For example, while in `Type`, the fields carry an optional @column annotation for the
//! column name, here that information is encoded into an attribute of `ResolvedType`.
//! If no @column is provided, the encoded information is set to an appropriate default value.

use std::collections::{HashMap, HashSet};

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use postgres_core_model::types::EntityRepresentation;
use serde::{Deserialize, Serialize};

use super::{
    access_builder::{ResolvedAccess, build_access},
    naming::{ToPlural, ToTableName},
};
use crate::{
    resolved_type::{
        ExplicitTypeHint, ResolvedCompositeType, ResolvedEnumType, ResolvedField,
        ResolvedFieldDefault, ResolvedFieldType, ResolvedType, SerializableTypeHint,
    },
    type_provider::{PRIMITIVE_TYPE_PROVIDER_REGISTRY, validate_hint_annotations},
};
use core_model::{
    mapped_arena::MappedArena,
    primitive_type::{self},
    types::{FieldType, Named},
};
use core_model_builder::{
    ast::ast_types::{
        AstAnnotation, AstAnnotationParams, AstExpr, AstField, AstFieldDefault,
        AstFieldDefaultKind, AstFieldType, AstModel, AstModelKind, default_span,
    },
    builder::resolved_builder::{AnnotationMapHelper, compute_fragment_fields},
    error::ModelBuildingError,
    typechecker::{
        Typed,
        typ::{Module, Type, TypecheckedSystem},
    },
};
use exo_sql::SchemaObjectName;

use heck::ToSnakeCase;

const DEFAULT_FN_AUTO_INCREMENT: &str = "autoIncrement";
const DEFAULT_FN_CURRENT_TIME: &str = "now";
const DEFAULT_FN_GENERATE_UUID: &str = "generate_uuid";
const DEFAULT_FN_UUID_GENERATE_V4: &str = "uuidGenerateV4";

/// Represents the different ways a field's column name(s) can be configured
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnMapping {
    /// Single column name for simple fields: @column("custom_name")
    Single(String),
    /// Mapping for composite types: @column(mapping={field1: "col1", field2: "col2"})
    Map(HashMap<String, String>),
}

/// Consume typed-checked types and build resolved types
pub fn build(
    typechecked_system: &TypecheckedSystem,
) -> Result<MappedArena<ResolvedType>, ModelBuildingError> {
    let mut errors = Vec::new();

    let resolved_system = resolve(typechecked_system, &mut errors)?;

    if errors.is_empty() {
        Ok(resolved_system)
    } else {
        Err(ModelBuildingError::Diagnosis(errors))
    }
}

fn resolve_field_type(typ: &Type, types: &MappedArena<Type>) -> FieldType<ResolvedFieldType> {
    match typ {
        Type::Optional(underlying) => {
            FieldType::Optional(Box::new(resolve_field_type(underlying.as_ref(), types)))
        }
        Type::Reference(id) => FieldType::Plain(ResolvedFieldType {
            type_name: types[*id].get_underlying_typename(types).unwrap(),
            is_primitive: matches!(types[*id], Type::Primitive(_) | Type::Enum(_)),
        }),
        Type::Set(underlying) | Type::Array(underlying) => {
            FieldType::List(Box::new(resolve_field_type(underlying.as_ref(), types)))
        }
        _ => todo!("Unsupported field type"),
    }
}

fn resolve(
    typechecked_system: &TypecheckedSystem,
    errors: &mut Vec<Diagnostic>,
) -> Result<MappedArena<ResolvedType>, ModelBuildingError> {
    let mut resolved_postgres_types: MappedArena<ResolvedType> = MappedArena::default();

    for (_, typ) in typechecked_system.types.iter() {
        // Adopt the primitive types as a PostgresType
        if let Type::Primitive(pt) = typ {
            resolved_postgres_types.add(&pt.name(), ResolvedType::Primitive(pt.clone()));
        }
    }

    for (_, Module(module)) in typechecked_system.modules.iter() {
        // Process each persistent type to create a PostgresType
        let module_annotation = module.annotations.get("postgres");

        let module_schema_name = match module_annotation {
            Some(a) => {
                if let AstAnnotationParams::Map(map, _) = a {
                    map.get("schema").map(|s| s.as_string())
                } else {
                    None
                }
            }
            None => continue,
        };

        let module_managed: Option<bool> = module_annotation.and_then(|a| {
            if let AstAnnotationParams::Map(map, _) = a {
                map.get("managed").map(|s| s.as_boolean())
            } else {
                None
            }
        });

        for typ in module.types.iter() {
            if let Some(Type::Composite(ct)) = typechecked_system.types.get_by_key(&typ.name) {
                resolve_composite_type(
                    ct,
                    &module_schema_name,
                    module_managed,
                    typechecked_system,
                    &mut resolved_postgres_types,
                    errors,
                )
                .map_err(|e| ModelBuildingError::Diagnosis(vec![e]))?;
            }
        }

        for typ in module.enums.iter() {
            if let Some(Type::Enum(et)) = typechecked_system.types.get_by_key(&typ.name) {
                resolved_postgres_types.add(
                    &et.name,
                    ResolvedType::Enum(ResolvedEnumType {
                        name: et.name.clone(),
                        fields: et.fields.iter().map(|f| f.name.clone()).collect(),
                        enum_name: SchemaObjectName::new(
                            et.name.to_snake_case(),
                            module_schema_name.as_deref(),
                        ),
                        doc_comments: et.doc_comments.clone(),
                        span: et.span,
                    }),
                );
            }
        }
    }

    Ok(resolved_postgres_types)
}

fn resolve_composite_type(
    ct: &AstModel<Typed>,
    module_schema_name: &Option<String>,
    module_managed: Option<bool>,
    typechecked_system: &TypecheckedSystem,
    resolved_postgres_types: &mut MappedArena<ResolvedType>,
    errors: &mut Vec<Diagnostic>,
) -> Result<(), Diagnostic> {
    if ct.kind == AstModelKind::Type {
        let plural_annotation_value = ct
            .annotations
            .get("plural")
            .map(|p| p.as_single().as_string());

        let table_annotation = ct.annotations.annotations.get("table");

        let TableInfo {
            name: table_name,
            schema: schema_name,
            managed: table_managed,
        } = extract_table_annotation(table_annotation, &ct.name, plural_annotation_value.clone())?;

        // If the table didn't specify a schema, use the module schema
        let schema_name = module_schema_name.clone().or(schema_name);

        // If there is an explicit table managed attribute, that takes precedence.
        // Otherwise, if there is an explicit module managed attribute, use that.
        // Otherwise, default to managed.
        let table_managed = match (table_managed, module_managed) {
            (Some(table_managed), _) => table_managed,
            (None, Some(module_managed)) => module_managed,
            (None, None) => true,
        };

        let representation = if ct.annotations.contains("json") {
            EntityRepresentation::Json
        } else if table_managed {
            EntityRepresentation::Managed
        } else {
            EntityRepresentation::NotManaged
        };

        let access_annotation = ct.annotations.get("access");

        let is_json = representation == EntityRepresentation::Json;

        if is_json && access_annotation.is_some() {
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "Cannot use @access for type {}. Json types behave like a primitive (and thus have always-allowed access)",
                    ct.name
                ),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: ct.span,
                    style: SpanStyle::Primary,
                    label: None,
                }],
            });
        }

        let access = if is_json {
            // As if the user has annotated with `access(true)`
            ResolvedAccess {
                default: Some(AstExpr::BooleanLiteral(true, default_span())),
                ..Default::default()
            }
        } else {
            build_access(access_annotation)
        };
        let name = ct.name.clone();
        let plural_name = plural_annotation_value.unwrap_or_else(|| ct.name.to_plural()); // fallback to automatically pluralizing name

        let resolved_fields =
            resolve_composite_type_fields(ct, is_json, table_managed, typechecked_system, errors);

        resolved_postgres_types.add(
            &ct.name,
            ResolvedType::Composite(ResolvedCompositeType {
                name,
                plural_name: plural_name.clone(),
                representation,
                fields: resolved_fields,
                table_name: SchemaObjectName {
                    name: table_name,
                    schema: schema_name,
                },
                access: access.clone(),
                doc_comments: ct.doc_comments.clone(),
                span: ct.span,
            }),
        );
    }

    Ok(())
}

fn resolve_composite_type_fields(
    ct: &AstModel<Typed>,
    is_json: bool,
    table_managed: bool,
    typechecked_system: &TypecheckedSystem,
    errors: &mut Vec<Diagnostic>,
) -> Vec<ResolvedField> {
    let fragment_fields = compute_fragment_fields(ct, errors, typechecked_system);

    ct.fields
        .iter()
        .chain(fragment_fields.iter().cloned())
        .flat_map(|field| {
            let update_sync = field.annotations.contains("update");
            let readonly = field.annotations.contains("readonly");

            let access_annotation = field.annotations.get("access");

            if is_json && access_annotation.is_some() {
                errors.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "Cannot use @access for field '{}' in a type with a '@json' annotation",
                        field.name
                    ),
                    code: Some("C000".to_string()),
                    spans: vec![SpanLabel {
                        span: field.span,
                        style: SpanStyle::Primary,
                        label: None,
                    }],
                });
            }

            // For fields, by default, we assume the `access(true)` annotation
            let access = match access_annotation {
                Some(_) => build_access(access_annotation),
                None => ResolvedAccess {
                    default: AstExpr::BooleanLiteral(true, default_span()).into(),
                    ..Default::default()
                },
            };

            let column_info =
                compute_column_info(ct, field, &typechecked_system.types, table_managed);

            match column_info {
                Ok(ColumnInfo {
                    names: column_names,
                    self_column,
                    unique_constraints,
                    indices,
                    cardinality,
                }) => {
                    let typ = resolve_field_type(
                        &field.typ.to_typ(&typechecked_system.types),
                        &typechecked_system.types,
                    );

                    let default_value = field
                        .default_value
                        .as_ref()
                        .map(|v| resolve_field_default_type(v, &typ, errors));

                    Some(ResolvedField {
                        name: field.name.clone(),
                        typ,
                        column_names,
                        self_column,
                        is_pk: field.annotations.contains("pk"),
                        access,
                        type_hint: build_type_hint(field, &typechecked_system.types, errors),
                        unique_constraints,
                        indices,
                        cardinality,
                        default_value,
                        update_sync,
                        readonly,
                        doc_comments: field.doc_comments.clone(),
                        span: field.span,
                    })
                }
                Err(e) => {
                    errors.push(e);
                    None
                }
            }
        })
        .collect()
}

fn resolve_field_default_type(
    default_value: &AstFieldDefault<Typed>,
    field_type: &FieldType<ResolvedFieldType>,
    errors: &mut Vec<Diagnostic>,
) -> ResolvedFieldDefault {
    let field_underlying_type = field_type.name();

    match &default_value.kind {
        AstFieldDefaultKind::Value(expr) => ResolvedFieldDefault::Value(Box::new(expr.to_owned())),
        AstFieldDefaultKind::Function(fn_name, args) => match fn_name.as_str() {
            DEFAULT_FN_AUTO_INCREMENT => {
                if field_underlying_type != primitive_type::IntType::NAME {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: format!("{DEFAULT_FN_AUTO_INCREMENT}() can only be used on Ints"),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: default_value.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    });
                }

                match &args[..] {
                    [] => ResolvedFieldDefault::AutoIncrement(None),
                    [AstExpr::StringLiteral(sequence_name, _)] => {
                        // Split the sequence name by '.' and use the last part as the sequence name
                        match sequence_name.split('.').collect::<Vec<&str>>()[..] {
                            [schema, name] => ResolvedFieldDefault::AutoIncrement(Some(
                                SchemaObjectName::new(name.to_string(), Some(schema)),
                            )),
                            [name] => ResolvedFieldDefault::AutoIncrement(Some(
                                SchemaObjectName::new(name.to_string(), None),
                            )),
                            _ => {
                                errors.push(Diagnostic {
                                    level: Level::Error,
                                    message: format!(
                                        "{}() can have arguments of the form 'schema.name' or 'name'",
                                        DEFAULT_FN_AUTO_INCREMENT
                                    ),
                                    code: Some("C000".to_string()),
                                    spans: vec![SpanLabel {
                                        span: default_value.span,
                                        style: SpanStyle::Primary,
                                        label: None,
                                    }],
                                });
                                ResolvedFieldDefault::AutoIncrement(None)
                            }
                        }
                    }
                    _ => {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "{}() can only have 0 (for default serial) or 1 argument (for a custom sequence name like 'schema.name')",
                                DEFAULT_FN_AUTO_INCREMENT
                            ),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: default_value.span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        });
                        ResolvedFieldDefault::AutoIncrement(None)
                    }
                }
            }
            DEFAULT_FN_CURRENT_TIME => {
                if field_underlying_type != primitive_type::InstantType::NAME
                    && field_underlying_type != primitive_type::LocalDateType::NAME
                    && field_underlying_type != primitive_type::LocalTimeType::NAME
                    && field_underlying_type != primitive_type::LocalDateTimeType::NAME
                {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "{DEFAULT_FN_CURRENT_TIME}() can only be used for time-related types"
                        ),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: default_value.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    });
                }

                ResolvedFieldDefault::PostgresFunction("now()".to_string())
            }
            DEFAULT_FN_GENERATE_UUID => {
                if field_underlying_type != primitive_type::UuidType::NAME {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: format!("{DEFAULT_FN_GENERATE_UUID}() can only be used on Uuids"),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: default_value.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    });
                }

                ResolvedFieldDefault::PostgresFunction("gen_random_uuid()".to_string())
            }
            DEFAULT_FN_UUID_GENERATE_V4 => {
                if field_underlying_type != primitive_type::UuidType::NAME {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "{DEFAULT_FN_UUID_GENERATE_V4}() can only be used on Uuids"
                        ),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: default_value.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    });
                }

                ResolvedFieldDefault::PostgresFunction("uuid_generate_v4()".to_string())
            }
            _ => {
                errors.push(Diagnostic {
                    level: Level::Error,
                    message: format!("Unknown function specified for default value: {fn_name}"),
                    code: Some("C000".to_string()),
                    spans: vec![SpanLabel {
                        span: default_value.span,
                        style: SpanStyle::Primary,
                        label: Some("unknown function".to_string()),
                    }],
                });
                // Proceed with a reasonable value. Since we already reported an error, this is not going to be used.
                ResolvedFieldDefault::PostgresFunction(fn_name.to_string())
            }
        },
    }
}

fn build_type_hint(
    field: &AstField<Typed>,
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
) -> Option<SerializableTypeHint> {
    let type_name = field.typ.get_underlying_typename(types)?;

    let explicit_dbtype_hint = field
        .annotations
        .get("dbtype")
        .map(|p| p.as_single().as_string())
        .map(|s| {
            SerializableTypeHint(Box::new(ExplicitTypeHint {
                dbtype: s.to_uppercase(),
            }))
        });

    let primitive_hint = {
        let type_hint_provider = PRIMITIVE_TYPE_PROVIDER_REGISTRY.get(type_name.as_str())?;
        validate_hint_annotations(field, *type_hint_provider, errors);
        type_hint_provider.compute_type_hint(field, errors)
    };

    // Validate that we don't have conflicting hints
    if explicit_dbtype_hint.is_some() && primitive_hint.is_some() {
        errors.push(Diagnostic {
            level: Level::Error,
            message: format!(
                "Cannot specify both @dbtype and a primitive specific hint for {}",
                field.name
            ),
            code: Some("C000".to_string()),
            spans: vec![SpanLabel {
                span: field.span,
                style: SpanStyle::Primary,
                label: None,
            }],
        });
    }

    // Return the appropriate hint (explicit takes precedence)
    explicit_dbtype_hint.or(primitive_hint)
}
struct ColumnInfo {
    names: Vec<String>,
    self_column: bool,
    unique_constraints: Vec<String>,
    indices: Vec<String>,
    cardinality: Option<Cardinality>,
}

fn compute_unique_constraints(field: &AstField<Typed>) -> Result<Vec<String>, Diagnostic> {
    match field.annotations.get("unique") {
        None => Ok(vec![]),
        Some(p) => match p {
            AstAnnotationParams::Single(expr, _) => match expr {
                AstExpr::StringLiteral(string, _) => Ok(vec![string.clone()]),
                AstExpr::StringList(string_list, _) => Ok(string_list.clone()),
                _ => Err(Diagnostic {
                    level: Level::Error,
                    message: "Not a string nor a string list when specifying unique".to_string(),
                    code: Some("C000".to_string()),
                    spans: vec![SpanLabel {
                        span: field.span,
                        style: SpanStyle::Primary,
                        label: None,
                    }],
                }),
            },
            AstAnnotationParams::None => Ok(vec![field.name.clone()]),
            AstAnnotationParams::Map(_, _) => Err(Diagnostic {
                level: Level::Error,
                message: "Cannot specify a map when specifying unique".to_string(),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: field.span,
                    style: SpanStyle::Primary,
                    label: None,
                }],
            }),
        },
    }
}

fn compute_indices(
    field: &AstField<Typed>,
    enclosing_type: &AstModel<Typed>,
) -> Result<Vec<String>, Diagnostic> {
    let index_annotation = field.annotations.get("index");

    match index_annotation {
        None => Ok(vec![]),
        Some(p) => match p {
            AstAnnotationParams::Single(expr, _) => match expr {
                AstExpr::StringLiteral(string, _) => Ok(vec![string.clone()]),
                AstExpr::StringList(string_list, _) => Ok(string_list.clone()),
                _ => Err(Diagnostic {
                    level: Level::Error,
                    message: "Not a string nor a string list when specifying index".to_string(),
                    code: Some("C000".to_string()),
                    spans: vec![SpanLabel {
                        span: field.span,
                        style: SpanStyle::Primary,
                        label: None,
                    }],
                }),
            },
            AstAnnotationParams::None => {
                let index_computed_name =
                    format!("{}_{}_idx", enclosing_type.name, field.name).to_ascii_lowercase();
                Ok(vec![index_computed_name.clone()])
            }
            AstAnnotationParams::Map(_, _) => Err(Diagnostic {
                level: Level::Error,
                message: "Cannot specify a map when specifying index".to_string(),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: field.span,
                    style: SpanStyle::Primary,
                    label: None,
                }],
            }),
        },
    }
}

fn compute_column_info(
    enclosing_type: &AstModel<Typed>,
    field: &AstField<Typed>,
    types: &MappedArena<Type>,
    table_managed: bool,
) -> Result<ColumnInfo, Diagnostic> {
    let unique_constraints = compute_unique_constraints(field)?;
    let indices = compute_indices(field, enclosing_type)?;
    let update_sync = field.annotations.contains("update");
    let readonly = field.annotations.contains("readonly");

    if (update_sync || readonly) && field.default_value.is_none() && table_managed {
        return Err(Diagnostic {
            level: Level::Error,
            message: "Fields with @readonly or @update must have a default value".to_string(),
            code: Some("C000".to_string()),
            spans: vec![SpanLabel {
                span: field.span,
                style: SpanStyle::Primary,
                label: None,
            }],
        });
    }

    let user_supplied_column_mapping = column_annotation_mapping(field);

    // Validate column mapping once to avoid duplicate errors
    if let Some(ColumnMapping::Map(mapping)) = &user_supplied_column_mapping {
        let field_base_type = match &field.typ {
            AstFieldType::Optional(inner_typ) => inner_typ.as_ref(),
            _ => &field.typ,
        };
        let field_type = field_base_type.to_typ(types).deref(types);

        if let Type::Composite(ct) = field_type {
            // Pre-compute field names for efficient lookups and simplify the code
            let type_field_names: HashSet<&String> = ct.fields.iter().map(|f| &f.name).collect();

            // Collect all primary key field names from the target type
            let pk_field_names: Vec<&String> = ct
                .fields
                .iter()
                .filter(|f| f.annotations.contains("pk"))
                .map(|f| &f.name)
                .collect();

            // Check if all mapping keys correspond to actual fields in the target type
            for (mapping_key, _) in mapping.iter() {
                if !type_field_names.contains(mapping_key) {
                    return Err(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "Field '{}' specified in column mapping does not exist in type '{}'",
                            mapping_key, ct.name
                        ),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: field.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    });
                }
            }

            // Check if all primary key fields are included in the mapping
            for pk_field_name in &pk_field_names {
                if !mapping.contains_key(*pk_field_name) {
                    return Err(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "Primary key field '{}' from type '{}' is missing in the column mapping",
                            pk_field_name, ct.name
                        ),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: field.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    });
                }
            }
        }
    }

    let compute_column_name = |field_name: &str| match &user_supplied_column_mapping {
        Some(ColumnMapping::Single(name)) => name.clone(),
        _ => field_name.to_snake_case(),
    };

    let id_column_names = |field: &AstField<Typed>| -> Result<Vec<String>, Diagnostic> {
        let user_supplied_column_mapping = column_annotation_mapping(field);

        // Handle simple column name for non-composite types
        if let Some(ColumnMapping::Single(name)) = user_supplied_column_mapping {
            return Ok(vec![name]);
        }

        let field_base_type = match &field.typ {
            AstFieldType::Optional(inner_typ) => inner_typ.as_ref(),
            _ => &field.typ,
        };
        let field_type = field_base_type.to_typ(types).deref(types);

        let base_name = field.name.to_snake_case();

        if let Type::Composite(ct) = field_type {
            // Validation is already done upfront, no need to repeat it here

            Ok(ct
                .fields
                .iter()
                .filter_map(|f| {
                    if f.annotations.contains("pk") {
                        match &user_supplied_column_mapping {
                            Some(ColumnMapping::Map(mapping)) => {
                                // Use the mapping if provided
                                mapping.get(&f.name).cloned()
                            }
                            _ => {
                                // Use the default naming convention
                                Some(format!("{}_{}", base_name, f.name))
                            }
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>())
        } else {
            Ok(vec![])
        }
    };

    // we can treat Optional fields as their inner type for the purposes
    // of computing their default column name
    let field_base_type = match &field.typ {
        AstFieldType::Optional(inner_typ) => inner_typ.as_ref(),
        _ => &field.typ,
    };

    match field_base_type {
        AstFieldType::Optional(_) => {
            // We've already unwrapped any Optional. A nested optional (e.g. venue: Venue??) doesn't make sense in our model.
            Err(Diagnostic {
                level: Level::Error,
                message: "Cannot have optional of an optional".to_string(),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: field.span,
                    style: SpanStyle::Primary,
                    label: None,
                }],
            })
        }
        AstFieldType::Plain(..) => {
            match field_base_type.to_typ(types).deref(types) {
                Type::Composite(field_type) => {
                    if field_type.annotations.contains("json") {
                        return Ok(ColumnInfo {
                            names: vec![compute_column_name(&field.name)],
                            self_column: true,
                            unique_constraints,
                            indices,
                            cardinality: None,
                        });
                    }

                    let matching_field =
                        get_matching_field(field, enclosing_type, &field_type, types);

                    match &field.typ {
                        AstFieldType::Optional(_) => {
                            // If the field is optional, we need to look at the cardinality of the matching field to determine the column name.
                            //
                            // If the cardinality is `One` (thus forming a one-to-one relationship),
                            // we need to use the matching field's name as the basis for the column name.

                            // For example, if we have the following model, we will have a `user_id` column in the `memberships` table,
                            // but have no column in the `users` table:
                            //
                            // type User {
                            //     ...
                            //     membership: Membership?
                            // }
                            // type Membership {
                            //     ...
                            //     user: User
                            // }
                            //
                            // If the cardinality is `Unbounded`, then we need to use the field's name. For example, if we have
                            // the following model, we will have a `venue_id` column in the `concerts` table.
                            //
                            // type Concert {
                            //    ...
                            //    venue: Venue?
                            // }
                            // type Venue {
                            //    ...
                            //    concerts: Set<Concert>
                            // }

                            let matching_field_cardinality = match matching_field {
                                Ok(matching_field) => Ok(field_cardinality(&matching_field.typ)),
                                Err(_) => annotation_cardinality(field),
                            }?;

                            match matching_field_cardinality {
                                Cardinality::ZeroOrOne => Err(Diagnostic {
                                    level: Level::Error,
                                    message:
                                        "Both side of one-to-one relationship cannot be optional"
                                            .to_string(),
                                    code: Some("C000".to_string()),
                                    spans: vec![SpanLabel {
                                        span: field.span,
                                        style: SpanStyle::Primary,
                                        label: None,
                                    }],
                                }),
                                Cardinality::One => {
                                    if user_supplied_column_mapping.is_some() {
                                        Err(Diagnostic {
                                                    level: Level::Error,
                                                    message: "Cannot specify @column with the optional side of a one-to-one relationship"
                                                        .to_string(),
                                                    code: Some("C000".to_string()),
                                                    spans: vec![SpanLabel {
                                                        span: field.span,
                                                        style: SpanStyle::Primary,
                                                        label: None,
                                                    }],
                                                })
                                    } else {
                                        Ok(ColumnInfo {
                                            names: id_column_names(matching_field?)?,
                                            self_column: false,
                                            unique_constraints,
                                            indices,
                                            cardinality: Some(matching_field_cardinality),
                                        })
                                    }
                                }
                                Cardinality::Unbounded => Ok(ColumnInfo {
                                    names: id_column_names(field)?,
                                    self_column: true,
                                    unique_constraints,
                                    indices,
                                    cardinality: Some(matching_field_cardinality),
                                }),
                            }
                        }
                        AstFieldType::Plain(..) => {
                            let matching_field_cardinality = match matching_field {
                                Ok(matching_field) => Ok(field_cardinality(&matching_field.typ)),
                                Err(_) => annotation_cardinality(field),
                            }?;

                            let unique_constraints =
                                if matches!(matching_field_cardinality, Cardinality::ZeroOrOne) {
                                    // Add an explicit unique constraint to enforce one-to-one constraint
                                    vec![field.name.clone()]
                                } else {
                                    unique_constraints
                                };

                            Ok(ColumnInfo {
                                names: id_column_names(field)?,
                                self_column: true,
                                unique_constraints,
                                indices,
                                cardinality: Some(matching_field_cardinality),
                            })
                        }
                    }
                }
                Type::Set(typ) => {
                    if let Type::Composite(field_type) = typ.deref(types) {
                        // OneToMany
                        let matching_field =
                            get_matching_field(field, enclosing_type, &field_type, types)?;

                        let matching_field_cardinality = field_cardinality(&matching_field.typ);

                        if matching_field_cardinality == Cardinality::Unbounded {
                            let referring_type_name = &enclosing_type.name;
                            let referred_type_name = &field_type.name;
                            let suggested_linking_type_name =
                                if referring_type_name < referred_type_name {
                                    format!("{}{}", referring_type_name, referred_type_name)
                                } else {
                                    format!("{}{}", referred_type_name, referring_type_name)
                                };

                            // We don't support direct many-to-many relationships
                            Err(Diagnostic {
                                level: Level::Error,
                                message: format!(
                                    "Many-to-many relationships without a linking type are not supported. Consider adding a type such as '{suggested_linking_type_name}' to connect '{referring_type_name}' and '{referred_type_name}",
                                ),
                                code: Some("C000".to_string()),
                                spans: vec![SpanLabel {
                                    span: field.span,
                                    style: SpanStyle::Primary,
                                    label: None,
                                }],
                            })
                        } else if user_supplied_column_mapping.is_some() {
                            return Err(Diagnostic {
                                level: Level::Error,
                                message: "Cannot specify @column with a collection field"
                                    .to_string(),
                                code: Some("C000".to_string()),
                                spans: vec![SpanLabel {
                                    span: field.span,
                                    style: SpanStyle::Primary,
                                    label: None,
                                }],
                            });
                        } else {
                            Ok(ColumnInfo {
                                names: id_column_names(matching_field)?,
                                self_column: false,
                                unique_constraints,
                                indices,
                                cardinality: Some(matching_field_cardinality),
                            })
                        }
                    } else {
                        Err(Diagnostic {
                            level: Level::Error,
                            message: "Sets of non-composites are not supported (consider using an `Array` instead)".to_string(),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: field.span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        })
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
                        Ok(ColumnInfo {
                            names: vec![compute_column_name(&field.name)],
                            self_column: true,
                            unique_constraints,
                            indices,
                            cardinality: None,
                        })
                    } else {
                        Err(Diagnostic {
                            level: Level::Error,
                            message: "Arrays of non-primitives are not supported".to_string(),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: field.span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        })
                    }
                }
                _ => Ok(ColumnInfo {
                    names: vec![compute_column_name(&field.name)],
                    self_column: true,
                    unique_constraints,
                    indices,
                    cardinality: None,
                }),
            }
        }
    }
}

fn column_annotation_mapping(field: &AstField<Typed>) -> Option<ColumnMapping> {
    field.annotations.get("column").and_then(|annotation| {
        match annotation {
            // Handle the object literal syntax: @column(mapping={zip: "azip", city: "acity"})
            AstAnnotationParams::Map(map, _) => {
                if let Some(AstExpr::ObjectLiteral(object_map, _)) = map.get("mapping") {
                    // Extract string values from the object literal
                    let mut result = HashMap::new();
                    for (key, value) in object_map {
                        if let AstExpr::StringLiteral(string_value, _) = value {
                            result.insert(key.clone(), string_value.clone());
                        }
                    }
                    if result.is_empty() {
                        None
                    } else {
                        Some(ColumnMapping::Map(result))
                    }
                } else {
                    None
                }
            }
            AstAnnotationParams::Single(AstExpr::StringLiteral(s, _), _) => {
                Some(ColumnMapping::Single(s.clone()))
            }
            _ => None,
        }
    })
}

fn get_matching_field<'a>(
    field: &AstField<Typed>,
    enclosing_type: &AstModel<Typed>,
    field_type: &'a AstModel<Typed>,
    types: &MappedArena<Type>,
) -> Result<&'a AstField<Typed>, Diagnostic> {
    fn relation_field_name(field: &AstField<Typed>) -> Option<String> {
        field
            .annotations
            .get("relation")
            .map(|p| p.as_single().as_string())
    }

    // Look into the type of the field. For example, while considering the `mainVenue: Venue` field in `Concert` (the enclosing type), look into the `Venue` type.
    let matching_fields: Vec<_> = field_type
        .fields
        .iter()
        .filter(|candidate_field| {
            // The type of the field must match the enclosing type. For example, the type must match the `Concert` type (or its variation such as `Set<Concert>`)
            let type_matches = candidate_field
                .typ
                .to_typ(types)
                .get_underlying_typename(types)
                .as_ref()
                == Some(&enclosing_type.name);

            // Ensure that relation annotation matches the field name.
            let relation1_matches = match relation_field_name(candidate_field).as_ref() {
                Some(relation_field_name) => relation_field_name == &field.name,
                None => true,
            };
            // Check the other way around
            let relation2_matches = match relation_field_name(field).as_ref() {
                Some(relation_field_name) => relation_field_name == &candidate_field.name,
                None => true,
            };

            // Both ways must match
            let relation_field_name_matches = relation1_matches && relation2_matches;

            type_matches && relation_field_name_matches && *candidate_field != field
        })
        .collect();

    match &matching_fields[..] {
        [matching_field] => Ok(matching_field),
        [] => Err(Diagnostic {
            level: Level::Error,
            message: format!(
                "Could not find the matching field of the '{}' type '{}'. Ensure that there is only one field of that type or the '@relation' annotation specifies the matching field name.",
                enclosing_type.name, field.name
            ),
            code: Some("C000".to_string()),
            spans: vec![SpanLabel {
                span: field.span,
                style: SpanStyle::Primary,
                label: None,
            }],
        }),
        _ => Err(Diagnostic {
            level: Level::Error,
            message: format!(
                "Found multiple matching fields ({}) of the '{}' type when determining the matching column for '{}'. Consider using the `@relation` annotation to resolve this ambiguity.",
                matching_fields
                    .into_iter()
                    .map(|f| format!("'{}'", f.name))
                    .collect::<Vec<_>>()
                    .join(", "),
                enclosing_type.name,
                field.name
            ),
            code: Some("C000".to_string()),
            spans: vec![SpanLabel {
                span: field.span,
                style: SpanStyle::Primary,
                label: None,
            }],
        }),
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Cardinality {
    ZeroOrOne,
    One,
    Unbounded,
}

fn field_cardinality(field_type: &AstFieldType<Typed>) -> Cardinality {
    match field_type {
        AstFieldType::Optional(underlying) => {
            let underlying_cardinality = field_cardinality(underlying);
            if underlying_cardinality == Cardinality::Unbounded {
                Cardinality::Unbounded
            } else {
                Cardinality::ZeroOrOne
            }
        }
        AstFieldType::Plain(_, name, ..) => {
            if name == "Set" {
                Cardinality::Unbounded
            } else {
                Cardinality::One
            }
        }
    }
}

fn annotation_cardinality(field: &AstField<Typed>) -> Result<Cardinality, Diagnostic> {
    let many_to_one = field.annotations.contains("manyToOne");
    let one_to_one = field.annotations.contains("oneToOne");

    if one_to_one && many_to_one {
        Err(Diagnostic {
            level: Level::Error,
            message: "Cannot specify both @oneToOne and @manyToOne on the same field".to_string(),
            code: Some("C000".to_string()),
            spans: vec![SpanLabel {
                span: field.span,
                style: SpanStyle::Primary,
                label: None,
            }],
        })
    } else if many_to_one {
        Ok(Cardinality::Unbounded)
    } else if one_to_one {
        // The field itself is a plain type, so the other side's cardinality is optional (ZeroOrOne)
        if matches!(field.typ, AstFieldType::Plain(..)) {
            Ok(Cardinality::ZeroOrOne)
        } else {
            Err(Diagnostic {
                level: Level::Error,
                message: "Cannot specify @oneToOne on an optional field. Either specify @manyToOne or include a matching field in the other type.".to_string(),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: field.span,
                    style: SpanStyle::Primary,
                    label: None,
                }],
            })
        }
    } else {
        Err(Diagnostic {
                level: Level::Error,
                message: "Cannot determine cardinality of field. Either specify @oneToOne or @manyToOne on the field, or add a matching field to the other type.".to_string(),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: field.span,
                    style: SpanStyle::Primary,
                    label: None,
                }],
            })
    }
}

struct TableInfo {
    name: String,
    schema: Option<String>,
    managed: Option<bool>,
}

/// Given parameters for `@table(name=<table-name>, schema=<schema-name>)` extract table and schema name.
///
/// If a single string is provided (for example, `@table("t_name")), it is assumed to be the table name and the schema name is assumed to be `public`.
/// If a map is provided (for example, `@table(name="t_name", schema="s_name")`), the table name is extracted from the `name` key and the schema name from the `schema` key.
/// If a map is provided with only one key (for example, `@table(name="t_name")`), the table name is extracted from the key and the schema name is assumed to be `public`.
///
///
/// If no parameters are provided, the table name is derived from the type name and the schema name is assumed to be `public`.
///
fn extract_table_annotation(
    table_annotation: Option<&AstAnnotation<Typed>>,
    type_name: &str,
    plural_annotation_value: Option<String>,
) -> Result<TableInfo, Diagnostic> {
    let default_table_name = || type_name.table_name(plural_annotation_value.clone());

    match table_annotation {
        Some(table_annotation) => match &table_annotation.params {
            AstAnnotationParams::Single(value, _) => Ok(TableInfo {
                name: value.as_string(),
                schema: None,
                managed: None,
            }),
            AstAnnotationParams::Map(m, _) => {
                let name = m
                    .get("name")
                    .map(|value| value.as_string())
                    .unwrap_or_else(default_table_name);
                let schema = m.get("schema").cloned().map(|value| value.as_string());
                let managed = m.get("managed").cloned().map(|value| value.as_boolean());

                Ok(TableInfo {
                    name,
                    schema,
                    managed,
                })
            }
            AstAnnotationParams::None => Err(Diagnostic {
                level: Level::Error,
                message: "The `@table` annotation must not be empty".to_string(),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: table_annotation.span,
                    style: SpanStyle::Primary,
                    label: None,
                }],
            }),
        },
        None => {
            let name = default_table_name();
            Ok(TableInfo {
                name: name.clone(),
                schema: None,
                managed: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::create_resolved_system_from_src;

    use multiplatform_test::multiplatform_test;
    use std::fs::File;

    macro_rules! assert_resolved {
        ($src:expr, $fn_name:expr) => {
            let resolved = create_resolved_system_from_src($src).unwrap();
            insta::with_settings!({sort_maps => true, prepend_module_to_snapshot => false}, {
                #[cfg(target_family = "wasm")]
                {
                    let expected = include_str!(concat!("./snapshots/", $fn_name, ".snap"));
                    let split_expected = expected.split("---\n").skip(2).collect::<Vec<&str>>().join("---");
                    let serialized = insta::_macro_support::serialize_value(
                        &resolved,
                        insta::_macro_support::SerializationFormat::Yaml,
                    );
                    assert_eq!(split_expected, serialized);
                }

                #[cfg(not(target_family = "wasm"))]
                {

                    insta::assert_yaml_snapshot!(resolved)
                }
            })
        };
    }

    macro_rules! assert_resolved_err {
        ($src:expr, $error_string:expr) => {
            let system = create_resolved_system_from_src($src);
            assert_eq!(system.is_err(), true, $error_string);
        };
    }

    #[test]
    fn with_annotations() {
        File::create("bar.js").unwrap();

        assert_resolved!(
            r#"
        @postgres
        module ConcertModule {
            @table("custom_concerts")
            type Concert {
              @pk @dbtype("bigint") @column("custom_id") id: Int = autoIncrement() 
              @column("custom_title") @maxLength(12) title: String
              @column("custom_venue_id") venue: Venue 
              @range(min=0, max=300) reserved: Int 
              @precision(4) time: Instant 
              @precision(10) @scale(2) price: Decimal
            }
        
            @table("venues")
            @plural("Venuess")
            type Venue {
              @pk @column("custom_id") id: Int = autoIncrement() 
              @column("custom_name") name: String 
              concerts: Set<Concert> 
              @bits16 capacity: Int
              @singlePrecision latitude: Float
            }       
        }

        @deno("bar.js")
        module Foo {
            export query qux(@inject exograph: Exograph, x: Int, y: String): Int
            mutation quuz(): String
        }
        "#,
            "with_annotations"
        );
    }

    #[multiplatform_test]
    fn with_defaults() {
        // Note the swapped order between @pk and @dbtype to assert that our parsing logic permits any order
        assert_resolved!(
            r#"
        @postgres
        module ConcertModule {
            type Concert {
                @dbtype("BIGINT") @pk  id: Int = autoIncrement() 
              title: String 
              @unique("unique_concert") venue: Venue 
              attending: Array<String>
              seating: Array<Array<Boolean>>
            }

            type Venue             {
              @pk @dbtype("BIGINT") id: Int  = autoIncrement() 
              name:String 
              concerts: Set<Concert> 
            }        
        }
        "#,
            "with_defaults"
        );
    }

    #[multiplatform_test]
    fn with_optional_fields() {
        assert_resolved!(
            r#"
        @postgres
        module ConcertModule {
            type Concert {
              @pk id: Int = autoIncrement() 
              title: String 
              venue: Venue? 
              icon: Blob?
            }

            type Venue {
              @pk id: Int = autoIncrement()
              name: String
              @column("custom_address") address: String? 
              concerts: Set<Concert>?
            }    
        }
        "#,
            "with_optional_fields"
        );
    }

    #[test]
    fn with_access() {
        File::create("logger.js").unwrap();

        assert_resolved!(
            r#"
        context AuthContext {
            @jwt("role") role: String 
        }
        
        @postgres
        module ConcertModule {
            @access(AuthContext.role == "ROLE_ADMIN" || self.public)
            type Concert {
              @pk id: Int = autoIncrement() 
              title: String
              public: Boolean
            }      


            @access(true)
            type Venue {
              @pk id: Int = autoIncrement() 
              name: String
            }   

            @access(false)
            type Artist {
              @pk id: Int = autoIncrement() 
              name: String
            }  
        }

        @deno("logger.js")
        module Logger {
            @access(AuthContext.role == "ROLE_ADMIN")
            export query log(@inject exograph: Exograph): Boolean
        }
        "#,
            "with_access"
        );
    }

    #[multiplatform_test]
    fn with_access_default_values() {
        assert_resolved!(
            r#"
        context AuthContext {
            @jwt role: String
        }
        
        @postgres
        module ConcertModule {
            @access(AuthContext.role == "ROLE_ADMIN" || self.public)
            type Concert {
              @pk id: Int = autoIncrement() 
              title: String
              public: Boolean
            }      
        }
        "#,
            "with_access_default_values"
        );
    }

    #[multiplatform_test]
    fn field_name_variations() {
        assert_resolved!(
            r#"
        @postgres
        module EntityModule {
            type Entity {
              @pk _id: Int = autoIncrement()
              title_main: String
              title_main1: String
              public1: Boolean
              PUBLIC2: Boolean
              foo123: Int
            }
        }"#,
            "field_name_variations"
        );
    }

    #[multiplatform_test]
    fn column_names_for_non_standard_relational_field_names() {
        assert_resolved!(
            r#"
        @postgres
        module ConcertModule {
            type Concert {
              @pk id: Int = autoIncrement()
              title: String
              venuex: Venue // non-standard name
              published: Boolean
            }
        
            type Venue {
              @pk id: Int = autoIncrement()
              name: String
              concerts: Set<Concert>
              published: Boolean
            }             
        }
        "#,
            "column_names_for_non_standard_relational_field_names"
        );
    }

    #[multiplatform_test]
    fn with_multiple_matching_field_no_column_annotation() {
        let src = r#"
        @postgres
        module ConcertModule {
            type Concert {
                @pk id: Int = autoIncrement() 
                title: String 
                //@column("ticket_office")
                ticket_office: Venue 
                //@column("main")
                main: Venue 
            }
          
            type Venue {
                @pk id: Int = autoIncrement() 
                name:String 
                //@column("ticket_office")
                ticket_events: Set<Concert> 
                //@column("main")
                main_events: Set<Concert> 
            }  
        }
        "#;

        let resolved = create_resolved_system_from_src(src);

        assert!(resolved.is_err());
    }

    #[multiplatform_test]
    fn column_annotation_on_collection_field() {
        let src = r#"
        @postgres
        module ConcertModule {
            type Concert {
                @pk id: Int = autoIncrement() 
                title: String 
                venue: Venue 
            }
          
            type Venue {
                @pk id: Int = autoIncrement() 
                name:String 
                @column("concerts") concerts: Set<Concert> 
            }  
        }
        "#;

        let resolved = create_resolved_system_from_src(src);

        assert!(resolved.is_err());
    }

    #[multiplatform_test]
    fn column_annotation_on_collectionn_field() {
        let src = r#"
        @postgres
        module ConcertModule {
            type Concert {
                @pk id: Int = autoIncrement() 
                title: String 
                venue: Venue 
            }
          
            type Venue {
                @pk id: Int = autoIncrement() 
                name:String 
                @column("concerts") concerts: Set<Concert>?
            }  
        }
        "#;

        let resolved = create_resolved_system_from_src(src);

        assert!(resolved.is_err());
    }

    #[multiplatform_test]
    fn column_annotation_on_optional_side_of_one_to_one() {
        let src = r#"
        @postgres
        module ConcertModule {
            type Concert {
                @pk id: Int = autoIncrement() 
                title: String 
                venue: Venue 
            }
          
            type Venue {
                @pk id: Int = autoIncrement() 
                name:String 
                @column("concert") concert: Concert?
            }  
        }
        "#;

        let resolved = create_resolved_system_from_src(src);

        assert!(resolved.is_err());
    }

    #[multiplatform_test]
    fn with_multiple_matching_field_with_column_annotation() {
        assert_resolved!(
            r#"
        @postgres
        module ConcertModule {
            type Concert {
                @pk id: Int = autoIncrement() 
                title: String  
                @column("ticket_office") ticket_office: Venue 
                @column("main") main: Venue 
            }
          
            type Venue {
                @pk id: Int = autoIncrement() 
                name:String 
                @relation("ticket_office") ticket_events: Set<Concert> 
                @relation("main") main_events: Set<Concert> 
            }  
        }
        "#,
            "with_multiple_matching_field_with_column_annotation"
        );
    }

    #[multiplatform_test]
    fn with_camel_case_model_and_fields() {
        assert_resolved!(
            r#"
        @postgres
        module ConcertModule {
            type ConcertInfo {
                @pk concertId: Int = autoIncrement() 
                mainTitle: String 
            }
        }
        "#,
            "with_camel_case_model_and_fields"
        );
    }

    #[multiplatform_test]
    fn non_public_schema() {
        // Both type and fields names are camel case, but the table and column should be defaulted to snake case
        assert_resolved!(
            r#"
        @postgres
        module Db {
            @table(schema="auth") // let the table name be derived from the type name
            type AuthSchemaTable {
                @pk id: Int = autoIncrement() 
                name: String 
            }

            @table(name="custom_table", schema="auth")
            type AuthSchemaTableWithCustomName {
                @pk id: Int = autoIncrement() 
                name: String 
            }
        }
        "#,
            "non_public_schema"
        );
    }

    #[multiplatform_test]
    fn many_to_many_without_linking_type() {
        assert_resolved_err!(
            r#"
        @postgres
        module ConcertModule {
            type Concert {
                @pk id: Int = autoIncrement() 
                title: String 
                artists: Set<Artist> 
            }
          
            type Artist {
                @pk id: Int = autoIncrement() 
                name:String 
                concerts: Set<Concert> 
            }  
        }
        "#,
            "Many-to-many relationships (both side non-optional) without a linking type should be rejected"
        );

        assert_resolved_err!(
            r#"
        @postgres
        module ConcertModule {
            type Concert {
                @pk id: Int = autoIncrement() 
                title: String 
                artists: Set<Artist>?
            }
          
            type Venue {
                @pk id: Int = autoIncrement() 
                name:String 
                concerts: Set<Concert> 
            }  
        }
        "#,
            "Many-to-many relationships (one side non-optional) without a linking type should be rejected"
        );

        assert_resolved_err!(
            r#"
        @postgres
        module ConcertModule {
            type Concert {
                @pk id: Int = autoIncrement() 
                title: String 
                artists: Set<Artist>?
            }
          
            type Venue {
                @pk id: Int = autoIncrement() 
                name:String 
                concerts: Set<Concert>?
            }  
        }
        "#,
            "Many-to-many relationships (both side optional) without a linking type should be rejected"
        );
    }

    #[multiplatform_test]
    fn column_mapping_validation() {
        // Test invalid field in mapping
        assert_resolved_err!(
            r#"
        @postgres
        module Database {
            type Member {
                @pk memberId: String
                @pk memberTenantId: String
                memberName: String?
            }

            type Membership {
                @pk membershipId: String
                @column(mapping={invalidField: "membership_member_id", memberTenantId: "membership_tenant_id"}) member: Member
            }
        }
        "#,
            "Field 'invalidField' specified in column mapping does not exist"
        );

        // Test missing primary key field in mapping
        assert_resolved_err!(
            r#"
        @postgres
        module Database {
            type Member {
                @pk memberId: String
                @pk memberTenantId: String
                memberName: String?
            }

            type Membership {
                @pk membershipId: String
                @column(mapping={memberTenantId: "membership_tenant_id"}) member: Member
            }
        }
        "#,
            "Primary key field 'memberId' from type 'Member' is missing"
        );

        // Test valid mapping
        assert_resolved!(
            r#"
        @postgres
        module Database {
            type Member {
                @pk memberId: String
                @pk memberTenantId: String
                memberName: String?
                memberships: Set<Membership>
            }

            type Membership {
                @pk membershipId: String
                @column(mapping={memberId: "membership_member_id", memberTenantId: "membership_tenant_id"}) member: Member
            }
        }
        "#,
            "column_mapping_validation"
        );
    }
}
