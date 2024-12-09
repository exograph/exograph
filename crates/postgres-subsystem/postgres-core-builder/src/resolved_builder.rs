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

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use postgres_core_model::types::EntityRepresentation;

use super::{
    access_builder::{build_access, ResolvedAccess},
    naming::{ToPlural, ToTableName},
};
use crate::resolved_type::{
    ResolvedCompositeType, ResolvedField, ResolvedFieldDefault, ResolvedFieldType, ResolvedType,
    ResolvedTypeHint,
};
use core_plugin_interface::{
    core_model::{
        mapped_arena::MappedArena,
        types::{FieldType, Named},
    },
    core_model_builder::{
        ast::ast_types::{
            default_span, AstAnnotationParams, AstExpr, AstField, AstFieldDefault,
            AstFieldDefaultKind, AstFieldType, AstModel, AstModelKind,
        },
        builder::resolved_builder::AnnotationMapHelper,
        error::ModelBuildingError,
        typechecker::{
            typ::{Module, Type, TypecheckedSystem},
            Typed,
        },
    },
};
use exo_sql::{PhysicalTableName, VectorDistanceFunction};

use heck::ToSnakeCase;

const DEFAULT_FN_AUTO_INCREMENT: &str = "autoIncrement";
const DEFAULT_FN_CURRENT_TIME: &str = "now";
const DEFAULT_FN_GENERATE_UUID: &str = "generate_uuid";

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
            is_primitive: matches!(types[*id], Type::Primitive(_)),
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
        if module.annotations.get("postgres").is_some() {
            for typ in module.types.iter() {
                if let Some(Type::Composite(ct)) = typechecked_system.types.get_by_key(&typ.name) {
                    if ct.kind == AstModelKind::Type {
                        let plural_annotation_value = ct
                            .annotations
                            .get("plural")
                            .map(|p| p.as_single().as_string());

                        let TableInfo {
                            name: table_name,
                            schema: schema_name,
                        } = extract_table_annotation(
                            ct.annotations.get("table"),
                            &ct.name,
                            plural_annotation_value.clone(),
                        );
                        let representation = if ct.annotations.contains("json") {
                            EntityRepresentation::Json
                        } else {
                            EntityRepresentation::Normal
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
                        let plural_name =
                            plural_annotation_value.unwrap_or_else(|| ct.name.to_plural()); // fallback to automatically pluralizing name

                        let resolved_fields = ct
                            .fields
                            .iter()
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
                                        default: AstExpr::BooleanLiteral(true, default_span())
                                            .into(),
                                        ..Default::default()
                                    },
                                };

                                let column_info =
                                    compute_column_info(ct, field, &typechecked_system.types);

                                match column_info {
                                    Ok(ColumnInfo {
                                        name: column_name,
                                        self_column,
                                        unique_constraints,
                                        indices,
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
                                            column_name,
                                            self_column,
                                            is_pk: field.annotations.contains("pk"),
                                            access,
                                            type_hint: build_type_hint(
                                                field,
                                                &typechecked_system.types,
                                                errors,
                                            ),
                                            unique_constraints,
                                            indices,
                                            default_value,
                                            update_sync,
                                            readonly,
                                            span: field.span,
                                        })
                                    }
                                    Err(e) => {
                                        errors.push(e);
                                        None
                                    }
                                }
                            })
                            .collect();

                        resolved_postgres_types.add(
                            &ct.name,
                            ResolvedType::Composite(ResolvedCompositeType {
                                name,
                                plural_name: plural_name.clone(),
                                representation,
                                fields: resolved_fields,
                                table_name: PhysicalTableName {
                                    name: table_name,
                                    schema: schema_name,
                                },
                                access: access.clone(),
                                span: ct.span,
                            }),
                        );
                    }
                }
            }
        }
    }

    Ok(resolved_postgres_types)
}

fn resolve_field_default_type(
    default_value: &AstFieldDefault<Typed>,
    field_type: &FieldType<ResolvedFieldType>,
    errors: &mut Vec<Diagnostic>,
) -> ResolvedFieldDefault {
    let field_underlying_type = field_type.name();

    match &default_value.kind {
        AstFieldDefaultKind::Value(expr) => ResolvedFieldDefault::Value(Box::new(expr.to_owned())),
        AstFieldDefaultKind::Function(fn_name, _args) => match fn_name.as_str() {
            DEFAULT_FN_AUTO_INCREMENT => {
                match field_underlying_type {
                    "Int" => {}
                    _ => {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "{DEFAULT_FN_AUTO_INCREMENT}() can only be used on Ints"
                            ),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: default_value.span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        });
                    }
                }

                ResolvedFieldDefault::AutoIncrement
            }
            DEFAULT_FN_CURRENT_TIME => {
                match field_underlying_type {
                    "Instant" | "LocalDate" | "LocalTime" | "LocalDateTime" => {}
                    _ => {
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
                }

                ResolvedFieldDefault::PostgresFunction("now()".to_string())
            }
            DEFAULT_FN_GENERATE_UUID => {
                match field_underlying_type {
                    "Uuid" => {}
                    _ => {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "{DEFAULT_FN_GENERATE_UUID}() can only be used on Uuids"
                            ),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: default_value.span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        });
                    }
                }

                ResolvedFieldDefault::PostgresFunction("gen_random_uuid()".to_string())
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
) -> Option<ResolvedTypeHint> {
    ////
    // Part 1: parse out and validate hints for each primitive
    ////

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

            let is_bits16 = field.annotations.contains("bits16");
            let is_bits32 = field.annotations.contains("bits32");
            let is_bits64 = field.annotations.contains("bits64");

            let bits_hint = match (is_bits16, is_bits32, is_bits64) {
                (true, false, false) => Some(16),
                (false, true, false) => Some(32),
                (false, false, true) => Some(64),
                (false, false, false) => None,
                _ => {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: "Cannot have more than one of @bits16, @bits32, @bits64"
                            .to_string(),
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
            let mut range_hint = None;
            if let Some(params) = field.annotations.get("range") {
                let min = params
                    .as_map()
                    .get("min")
                    .unwrap()
                    .as_string()
                    .parse::<f64>();
                let max = params
                    .as_map()
                    .get("max")
                    .unwrap()
                    .as_string()
                    .parse::<f64>();

                if min.is_err() || max.is_err() {
                    if min.is_err() {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: "Cannot parse @range 'min' as f64".to_string(),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: field.span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        });
                    }
                    if max.is_err() {
                        errors.push(Diagnostic {
                            level: Level::Error,
                            message: "Cannot parse @range 'max' as f64".to_string(),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: field.span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        });
                    }
                } else {
                    range_hint = Some((min.unwrap(), max.unwrap()));
                }
            };

            let is_single_precision = field.annotations.contains("singlePrecision");
            let is_double_precision = field.annotations.contains("doublePrecision");

            let bits_hint = match (is_single_precision, is_double_precision) {
                (true, false) => Some(24),
                (false, true) => Some(53),
                (false, false) => None,
                _ => {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: "Cannot have both @singlePrecision and @doublePrecision"
                            .to_string(),
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

            if bits_hint.is_some() || range_hint.is_some() {
                Some(ResolvedTypeHint::Float {
                    bits: bits_hint,
                    range: range_hint,
                })
            } else {
                None
            }
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
                errors.push(Diagnostic {
                    level: Level::Error,
                    message: "@scale is not allowed without specifying @precision".to_string(),
                    code: Some("C000".to_string()),
                    spans: vec![SpanLabel {
                        span: field.span,
                        style: SpanStyle::Primary,
                        label: None,
                    }],
                });
            }

            Some(ResolvedTypeHint::Decimal {
                precision: precision_hint,
                scale: scale_hint,
            })
        }
    };

    let string_hint = {
        let max_length_annotation = field
            .annotations
            .get("maxLength")
            .map(|p| p.as_single().as_number() as usize);

        // None if there is no maxLength annotation
        max_length_annotation.map(|max_length| ResolvedTypeHint::String { max_length })
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

    let vector_hint = if field.typ.get_underlying_typename(types).unwrap() == "Vector" {
        let size = field
            .annotations
            .get("size")
            .map(|p| p.as_single().as_number() as usize);

        let distance_function = field.annotations.get("distanceFunction").and_then(|p| {
            match VectorDistanceFunction::from_model_string(p.as_single().as_string().as_str()) {
                Ok(distance_function) => Some(distance_function),
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
            }
        });

        Some(ResolvedTypeHint::Vector {
            size,
            distance_function,
        })
    } else {
        None
    };

    let primitive_hints = vec![
        int_hint,
        float_hint,
        number_hint,
        string_hint,
        datetime_hint,
        vector_hint,
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
        .map(|hint| usize::from(hint.is_some()))
        .sum();

    let valid_primitive_hints_exist = number_of_valid_primitive_hints > 0;

    if explicit_dbtype_hint.is_some() && valid_primitive_hints_exist {
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

    if number_of_valid_primitive_hints > 1 {
        errors.push(Diagnostic {
            level: Level::Error,
            message: format!("Conflicting type hints specified for {}", field.name),
            code: Some("C000".to_string()),
            spans: vec![SpanLabel {
                span: field.span,
                style: SpanStyle::Primary,
                label: None,
            }],
        });
    }

    ////
    // Part 3: return appropriate hint
    ////

    if explicit_dbtype_hint.is_some() {
        explicit_dbtype_hint
    } else if number_of_valid_primitive_hints == 1 {
        match primitive_hints.into_iter().find(|hint| hint.is_some()) {
            Some(Some(hint)) => Some(hint),
            _ => {
                errors.push(Diagnostic {
                    level: Level::Error,
                    message: "Could not find a valid hint".to_string(),
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
    } else {
        None
    }
}
struct ColumnInfo {
    name: String,
    self_column: bool,
    unique_constraints: Vec<String>,
    indices: Vec<String>,
    // // Will this field be auto-updated by the system (through triggers, etc.) to its default value?
    // update_sync: bool,
}

fn compute_column_info(
    enclosing_type: &AstModel<Typed>,
    field: &AstField<Typed>,
    types: &MappedArena<Type>,
) -> Result<ColumnInfo, Diagnostic> {
    let user_supplied_column_name = field
        .annotations
        .get("column")
        .map(|p| p.as_single().as_string());

    let compute_column_name = |field_name: &str| {
        user_supplied_column_name
            .clone()
            .unwrap_or_else(|| field_name.to_snake_case())
    };

    let unique_constraints = field
        .annotations
        .get("unique")
        .map(|p| match p {
            AstAnnotationParams::Single(expr, _) => match expr {
                AstExpr::StringLiteral(string, _) => vec![string.clone()],
                AstExpr::StringList(string_list, _) => string_list.clone(),
                _ => panic!("Not a string nor a string list when specifying unique"),
            },
            AstAnnotationParams::None => vec![field.name.clone()],
            AstAnnotationParams::Map(_, _) => panic!(),
        })
        .unwrap_or_default();

    let indices = {
        field
            .annotations
            .get("index")
            .map(|p| match p {
                AstAnnotationParams::Single(expr, _) => match expr {
                    AstExpr::StringLiteral(string, _) => vec![string.clone()],
                    AstExpr::StringList(string_list, _) => string_list.clone(),
                    _ => panic!("Not a string nor a string list when specifying index"),
                },
                AstAnnotationParams::None => {
                    let index_computed_name =
                        format!("{}_{}_idx", enclosing_type.name, field.name).to_ascii_lowercase();
                    vec![index_computed_name.clone()]
                }
                AstAnnotationParams::Map(_, _) => panic!(),
            })
            .unwrap_or_default()
    };

    let update_sync = field.annotations.contains("update");
    let readonly = field.annotations.contains("readonly");

    if (update_sync || readonly) && field.default_value.is_none() {
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

    let id_column_name = |field_name: &str| {
        user_supplied_column_name
            .clone()
            .unwrap_or(format!("{}_id", field_name.to_snake_case()))
    };
    // we can treat Optional fields as their inner type for the purposes
    // of computing their default column name
    let field_base_type = match &field.typ {
        AstFieldType::Optional(inner_typ) => inner_typ.as_ref(),
        _ => &field.typ,
    };

    match field_base_type {
        AstFieldType::Plain(_, _, _, _, _) => {
            match field_base_type.to_typ(types).deref(types) {
                Type::Composite(field_type) => {
                    if field_type.annotations.contains("json") {
                        return Ok(ColumnInfo {
                            name: compute_column_name(&field.name),
                            self_column: true,
                            unique_constraints,
                            indices,
                        });
                    }
                    let matching_field =
                        get_matching_field(field, enclosing_type, &field_type, types);
                    let matching_field = match matching_field {
                        Ok(matching_field) => matching_field,
                        Err(err) => return Err(err),
                    };

                    let cardinality = field_cardinality(&matching_field.typ);

                    match &field.typ {
                        AstFieldType::Optional(_) => {
                            // If the field is optional, we need to look at the cardinality of the matching field in the type of
                            // the field.
                            //
                            // If the cardinality is One (thus forming a one-to-one relationship), then we need to use the matching field's name.
                            // For example, if we have the following model, we will have a `user_id` column in `memberships` table, but no column in the `users` table:
                            // type User {
                            //     ...
                            //     membership: Membership?
                            // }
                            // type Membership {
                            //     ...
                            //     user: User
                            // }
                            //
                            // If the cardinality is Unbounded, then we need to use the field's name. For example, if we have
                            // the following model, we will have a `venue_id` column in the `concerts` table.
                            // type Concert {
                            //    ...
                            //    venue: Venue?
                            // }
                            // type Venue {
                            //    ...
                            //    concerts: Set<Concert>
                            // }

                            match cardinality {
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
                                Cardinality::One => Ok(ColumnInfo {
                                    name: id_column_name(&matching_field.name),
                                    self_column: false,
                                    unique_constraints,
                                    indices,
                                }),
                                Cardinality::Unbounded => Ok(ColumnInfo {
                                    name: id_column_name(&field.name),
                                    self_column: true,
                                    unique_constraints,
                                    indices,
                                }),
                            }
                        }
                        _ => {
                            let unique_constraints =
                                if matches!(cardinality, Cardinality::ZeroOrOne) {
                                    // Add an explicit unique constraint to enforce one-to-one constraint
                                    vec![field.name.clone()]
                                } else {
                                    unique_constraints
                                };

                            Ok(ColumnInfo {
                                name: id_column_name(&field.name),
                                self_column: true,
                                unique_constraints,
                                indices,
                            })
                        }
                    }
                }
                Type::Set(typ) => {
                    if let Type::Composite(field_type) = typ.deref(types) {
                        // OneToMany
                        let matching_field =
                            get_matching_field(field, enclosing_type, &field_type, types);

                        let matching_field = match matching_field {
                            Ok(matching_field) => matching_field,
                            Err(err) => return Err(err),
                        };

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
                        } else {
                            Ok(ColumnInfo {
                                name: id_column_name(&matching_field.name),
                                self_column: false,
                                unique_constraints,
                                indices,
                            })
                        }
                    } else {
                        Err(Diagnostic {
                            level: Level::Error,
                            message: "Sets of non-composites are not supported".to_string(),
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
                            name: compute_column_name(&field.name),
                            self_column: true,
                            unique_constraints,
                            indices,
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
                    name: compute_column_name(&field.name),
                    self_column: true,
                    unique_constraints,
                    indices,
                }),
            }
        }
        AstFieldType::Optional(_) => {
            // we already unwrapped any Optional there may be
            // a nested Optional doesn't make sense
            Err(Diagnostic {
                level: Level::Error,
                message: "Cannot have Optional of an Optional".to_string(),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: field.span,
                    style: SpanStyle::Primary,
                    label: None,
                }],
            })
        }
    }
}

fn get_matching_field<'a>(
    field: &AstField<Typed>,
    enclosing_type: &AstModel<Typed>,
    field_type: &'a AstModel<Typed>,
    types: &MappedArena<Type>,
) -> Result<&'a AstField<Typed>, Diagnostic> {
    let user_supplied_column_name = field
        .annotations
        .annotations
        .get("column")
        .map(|p| p.params.as_single().as_string());

    let matching_fields: Vec<_> = field_type
        .fields
        .iter()
        .filter(|f| {
            // If the user supplied a column name, then we look for the corresponding field
            // with the same name. We still need to check if the field is the same type though.
            let field_column_annotation = f
                .annotations
                .get("column")
                .map(|p| p.as_single().as_string());

            let column_name_matches = user_supplied_column_name == field_column_annotation;
            let field_underlying_type = f.typ.to_typ(types);
            field_underlying_type
                .get_underlying_typename(types)
                .unwrap()
                == enclosing_type.name
                && column_name_matches
        })
        .collect();

    match &matching_fields[..] {
        [] => {
            Err(Diagnostic {
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
                }
            ],
        })},
        [matching_field] => Ok(matching_field),
        _ => {
            Err(
                Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "Found multiple matching fields ({}) of the '{}' type when determining the matching column for '{}'",
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
                }
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Cardinality {
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

struct TableInfo {
    name: String,
    schema: Option<String>,
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
    annotation_params: Option<&AstAnnotationParams<Typed>>,
    type_name: &str,
    plural_annotation_value: Option<String>,
) -> TableInfo {
    let default_table_name = || type_name.table_name(plural_annotation_value.clone());

    match annotation_params {
        Some(p) => match p {
            AstAnnotationParams::Single(value, _) => TableInfo {
                name: value.as_string(),
                schema: None,
            },
            AstAnnotationParams::Map(m, _) => {
                let name = m
                    .get("name")
                    .map(|value| value.as_string())
                    .unwrap_or_else(default_table_name);
                let schema = m.get("schema").cloned().map(|value| value.as_string());

                TableInfo { name, schema }
            }
            _ => panic!(),
        },
        None => {
            let name = default_table_name();
            TableInfo {
                name: name.clone(),
                schema: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use codemap::CodeMap;
    use multiplatform_test::multiplatform_test;

    use super::*;
    use builder::{load_subsystem_builders, parser, typechecker};
    use std::fs::File;

    fn create_resolved_system(src: &str) -> Result<MappedArena<ResolvedType>, ModelBuildingError> {
        let mut codemap = CodeMap::new();
        let subsystem_builders = load_subsystem_builders(vec![
            Box::new(postgres_builder::PostgresSubsystemBuilder::default()),
            #[cfg(not(target_family = "wasm"))]
            Box::new(deno_builder::DenoSubsystemBuilder::default()),
        ])
        .unwrap();
        let parsed = parser::parse_str(src, &mut codemap, "input.exo")
            .map_err(|e| ModelBuildingError::Generic(format!("{e:?}")))?;
        let typechecked_system = typechecker::build(&subsystem_builders, parsed)
            .map_err(|e| ModelBuildingError::Generic(format!("{e:?}")))?;

        build(&typechecked_system)
    }

    macro_rules! assert_resolved {
        ($src:expr, $fn_name:expr) => {
            let resolved = create_resolved_system($src).unwrap();
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
            let system = create_resolved_system($src);
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
              @column("custom_venue_id") concerts: Set<Concert> 
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
                id: Int  @autoIncrement @pk 
                name:String 
                //@column("ticket_office")
                ticket_events: Set<Concert> 
                //@column("main")
                main_events: Set<Concert> 
            }  
        }
        "#;

        let resolved = create_resolved_system(src);

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
                @column("ticket_office") ticket_events: Set<Concert> 
                @column("main") main_events: Set<Concert> 
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
}
