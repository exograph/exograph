//! Resolve types to consume and normalize annotations.
//!
//! For example, while in `Type`, the fields carry an optional @column annotation for the
//! column name, here that information is encoded into an attribute of `ResolvedType`.
//! If no @column is provided, the encoded information is set to an appropriate default value.

use super::naming::{ToPlural, ToTableName};
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_core_model_builder::builder::access_builder::build_access;
use payas_core_model_builder::builder::resolved_builder::{
    resolve_field_type, AnnotationMapHelper, ResolvedCompositeType, ResolvedCompositeTypeKind,
    ResolvedField, ResolvedFieldDefault, ResolvedFieldKind, ResolvedType, ResolvedTypeHint,
};
use payas_core_model_builder::typechecker::typ::Type;
use payas_core_model_builder::typechecker::Typed;
use payas_model::model::mapped_arena::MappedArena;

use super::{DEFAULT_FN_AUTOINCREMENT, DEFAULT_FN_CURRENT_TIME, DEFAULT_FN_GENERATE_UUID};
use heck::ToSnakeCase;
use payas_core_model_builder::ast::ast_types::{
    AstAnnotationParams, AstExpr, AstField, AstFieldDefault, AstFieldDefaultKind, AstFieldType,
    AstModel, AstModelKind,
};
use payas_core_model_builder::error::ModelBuildingError;
use serde::{Deserialize, Serialize};

/// Consume typed-checked types and build resolved types
#[derive(Deserialize, Serialize)]
pub struct ResolvedDatabaseSystem {
    pub database_types: MappedArena<ResolvedType>,
}

impl ToPlural for ResolvedCompositeType {
    fn to_singular(&self) -> String {
        self.name.clone()
    }

    fn to_plural(&self) -> String {
        self.plural_name.clone()
    }
}

pub fn build(types: &MappedArena<Type>) -> Result<ResolvedDatabaseSystem, ModelBuildingError> {
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
) -> Result<ResolvedDatabaseSystem, ModelBuildingError> {
    Ok(ResolvedDatabaseSystem {
        database_types: resolve_persistent_types(types, errors)?,
    })
}

fn resolve_persistent_types(
    types: &MappedArena<Type>,
    errors: &mut Vec<Diagnostic>,
) -> Result<MappedArena<ResolvedType>, ModelBuildingError> {
    let mut resolved_database_types: MappedArena<ResolvedType> = MappedArena::default();

    for (_, typ) in types.iter() {
        if let Type::Composite(ct) = typ {
            if ct.kind == AstModelKind::Persistent {
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
                let name = ct.name.clone();
                let plural_name = plural_annotation_value.unwrap_or_else(|| ct.name.to_plural()); // fallback to automatically pluralizing name

                let resolved_fields = ct
                    .fields
                    .iter()
                    .flat_map(|field| {
                        let resolved_kind = {
                            let column_info = compute_column_info(ct, field, types);

                            match column_info {
                                Ok(ColumnInfo {
                                    name: column_name,
                                    self_column,
                                    unique_constraints,
                                }) => Some(ResolvedFieldKind::Persistent {
                                    column_name,
                                    self_column,
                                    is_pk: field.annotations.contains("pk"),
                                    type_hint: build_type_hint(field, types),
                                    unique_constraints,
                                }),
                                Err(e) => {
                                    errors.push(e);
                                    None
                                }
                            }
                        };

                        resolved_kind.map(|kind| ResolvedField {
                            name: field.name.clone(),
                            typ: resolve_field_type(&field.typ.to_typ(types), types),
                            kind,
                            default_value: field
                                .default_value
                                .as_ref()
                                .map(resolve_field_default_type),
                        })
                    })
                    .collect();

                resolved_database_types.add(
                    &ct.name,
                    ResolvedType::Composite(ResolvedCompositeType {
                        name,
                        plural_name: plural_name.clone(),
                        fields: resolved_fields,
                        kind: ResolvedCompositeTypeKind::Persistent { table_name },
                        access: access.clone(),
                    }),
                );
            }
        }
    }

    Ok(resolved_database_types)
}

fn resolve_field_default_type(default_value: &AstFieldDefault<Typed>) -> ResolvedFieldDefault {
    match &default_value.kind {
        AstFieldDefaultKind::Value(expr) => ResolvedFieldDefault::Value(Box::new(expr.to_owned())),
        AstFieldDefaultKind::DatabaseFunction(func) => {
            ResolvedFieldDefault::DatabaseFunction(func.to_owned())
        }
        AstFieldDefaultKind::Function(fn_name, _args) => match fn_name.as_str() {
            DEFAULT_FN_AUTOINCREMENT => ResolvedFieldDefault::Autoincrement,
            DEFAULT_FN_CURRENT_TIME => ResolvedFieldDefault::DatabaseFunction("now()".to_string()),
            DEFAULT_FN_GENERATE_UUID => {
                ResolvedFieldDefault::DatabaseFunction("gen_random_uuid()".to_string())
            }
            _ => panic!(),
        },
    }
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

struct ColumnInfo {
    name: String,
    self_column: bool,
    unique_constraints: Vec<String>,
}

fn compute_column_info(
    enclosing_type: &AstModel<Typed>,
    field: &AstField<Typed>,
    types: &MappedArena<Type>,
) -> Result<ColumnInfo, Diagnostic> {
    fn column_name(
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

        let unique_constraint_computed_name =
            format!("unique_constraint_{}_{}", enclosing_type.name, field.name)
                .to_ascii_lowercase();
        let unique_constraints = field
            .annotations
            .get("unique")
            .map(|p| match p {
                AstAnnotationParams::Single(expr, _) => match expr {
                    AstExpr::StringLiteral(string, _) => vec![string.clone()],
                    AstExpr::StringList(string_list, _) => string_list.clone(),
                    _ => panic!("Not a string nor a string list"),
                },
                AstAnnotationParams::None => vec![unique_constraint_computed_name.clone()],
                AstAnnotationParams::Map(_, _) => panic!(),
            })
            .unwrap_or_default();

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
            AstFieldType::Plain(_, _, _, _) => {
                match field_base_type.to_typ(types).deref(types) {
                    Type::Composite(field_model) => {
                        let matching_field =
                            get_matching_field(field, enclosing_type, &field_model, types);
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
                                // model User {
                                //     ...
                                //     membership: Membership?
                                // }
                                // model Membership {
                                //     ...
                                //     user: User
                                // }
                                //
                                // If the cardinality is Unbounded, then we need to use the field's name. For example, if we have
                                // the following model, we will have a `venue_id` column in the `concerts` table.
                                // model Concert {
                                //    ...
                                //    venue: Venue?
                                // }
                                // model Venue {
                                //    ...
                                //    concerts: Set<Concert>
                                // }

                                match cardinality {
                                    Cardinality::ZeroOrOne => {
                                        Err(Diagnostic {
                                        level: Level::Error,
                                        message: "Both side of one-to-one relationship cannot be optional".to_string(),
                                        code: Some("C000".to_string()),
                                        spans: vec![SpanLabel {
                                            span: field.span,
                                            style: SpanStyle::Primary,
                                            label: None,
                                        }],
                                    })
                                    }
                                    Cardinality::One => Ok(ColumnInfo {
                                        name: id_column_name(&matching_field.name),
                                        self_column: false,
                                        unique_constraints
                                    }),
                                    Cardinality::Unbounded => Ok(ColumnInfo {
                                        name: id_column_name(&field.name),
                                        self_column: true,
                                        unique_constraints
                                    }),
                                }
                            }
                            _ => {
                                let unique_constraints =
                                    if matches!(cardinality, Cardinality::ZeroOrOne) {
                                        vec![unique_constraint_computed_name]
                                    } else {
                                        unique_constraints
                                    };

                                Ok(ColumnInfo {
                                    name: id_column_name(&field.name),
                                    self_column: true,
                                    unique_constraints,
                                })
                            }
                        }
                    }
                    Type::Set(typ) => {
                        if let Type::Composite(field_model) = typ.deref(types) {
                            // OneToMany
                            let matching_field =
                                get_matching_field(field, enclosing_type, &field_model, types);

                            let matching_field = match matching_field {
                                Ok(matching_field) => matching_field,
                                Err(err) => return Err(err),
                            };
                            Ok(ColumnInfo {
                                name: id_column_name(&matching_field.name),
                                self_column: false,
                                unique_constraints,
                            })
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

    column_name(enclosing_type, field, types)
}

fn get_matching_field<'a>(
    field: &AstField<Typed>,
    enclosing_type: &AstModel<Typed>,
    field_model: &'a AstModel<Typed>,
    types: &MappedArena<Type>,
) -> Result<&'a AstField<Typed>, Diagnostic> {
    let user_supplied_column_name = field
        .annotations
        .annotations
        .get("column")
        .map(|p| p.params.as_single().as_string());

    let matching_fields: Vec<_> = field_model
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
        AstFieldType::Plain(name, ..) => {
            if name == "Set" {
                Cardinality::Unbounded
            } else {
                Cardinality::One
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use codemap::CodeMap;

    use super::*;
    use crate::{parser, typechecker};
    use std::fs::File;

    #[test]
    fn with_annotations() {
        let src = r#"
        @table("custom_concerts")
        model Concert {
          id: Int = autoincrement() @pk @dbtype("bigint") @column("custom_id")
          title: String @column("custom_title") @length(12)
          venue: Venue @column("custom_venue_id")
          reserved: Int @range(min=0, max=300)
          time: Instant @precision(4)
          price: Decimal @precision(10) @scale(2)
        }
        
        @table("venues")
        @plural_name("Venuess")
        model Venue {
          id: Int = autoincrement() @pk @column("custom_id")
          name: String @column("custom_name")
          concerts: Set<Concert> @column("custom_venue_id")
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

        let resolved = create_resolved_system(src).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn with_defaults() {
        // Note the swapped order between @pk and @dbtype to assert that our parsing logic permits any order
        let src = r#"
        model Concert {
          id: Int = autoincrement() @dbtype("BIGINT") @pk 
          title: String 
          venue: Venue @unique("unique_concert")
          attending: Array<String>
          seating: Array<Array<Boolean>>
        }

        model Venue             {
          id: Int  = autoincrement() @pk @dbtype("BIGINT")
          name:String 
          concerts: Set<Concert> 
        }        
        "#;

        let resolved = create_resolved_system(src).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn with_optional_fields() {
        let src = r#"
        model Concert {
          id: Int = autoincrement() @pk 
          title: String 
          venue: Venue? 
          icon: Blob?
        }

        model Venue {
          id: Int = autoincrement() @pk
          name: String
          address: String? @column("custom_address")
          concerts: Set<Concert>?
        }    
        "#;

        let resolved = create_resolved_system(src).unwrap();

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
          id: Int = autoincrement() @pk 
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

        let resolved = create_resolved_system(src).unwrap();

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
          id: Int = autoincrement() @pk 
          title: String
          public: Boolean
        }      
        "#;

        let resolved = create_resolved_system(src).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn field_name_variations() {
        let src = r#"
        model Entity {
          _id: Int = autoincrement() @pk
          title_main: String
          title_main1: String
          public1: Boolean
          PUBLIC2: Boolean
          foo123: Int
        }"#;

        let resolved = create_resolved_system(src).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn column_names_for_non_standard_relational_field_names() {
        let src = r#"
        model Concert {
          id: Int = autoincrement() @pk
          title: String
          venuex: Venue // non-standard name
          published: Boolean
        }
        
        model Venue {
          id: Int = autoincrement() @pk
          name: String
          concerts: Set<Concert>
          published: Boolean
        }             
        "#;

        let resolved = create_resolved_system(src).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn with_multiple_matching_field_no_column_annotation() {
        let src = r#"
            model Concert {
                id: Int = autoincrement() @pk 
                title: String 
                ticket_office: Venue //@column("ticket_office")
                main: Venue //@column("main")
            }
          
            model Venue {
                id: Int  @autoincrement @pk 
                name:String 
                ticket_events: Set<Concert> //@column("ticket_office")
                main_events: Set<Concert> //@column("main")
            }  
        "#;

        let resolved = create_resolved_system(src);

        assert!(resolved.is_err());
    }

    #[test]
    fn with_multiple_matching_field_with_column_annotation() {
        let src = r#"
            model Concert {
                id: Int = autoincrement() @pk 
                title: String  
                ticket_office: Venue @column("ticket_office")
                main: Venue @column("main")
            }
          
            model Venue {
                id: Int = autoincrement() @pk 
                name:String 
                ticket_events: Set<Concert> @column("ticket_office")
                main_events: Set<Concert> @column("main")
            }  
        "#;

        let resolved = create_resolved_system(src).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    #[test]
    fn with_camel_case_model_and_fields() {
        let src = r#"
            model ConcertInfo {
                concertId: Int = autoincrement() @pk 
                mainTitle: String 
            }
        "#;

        // Both model and fields names are camel case, but the table and column should be defaulted to snake case
        let resolved = create_resolved_system(src).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(resolved);
        });
    }

    fn create_resolved_system(src: &str) -> Result<ResolvedSystem, ModelBuildingError> {
        let mut codemap = CodeMap::new();
        let parsed = parser::parse_str(src, &mut codemap, "input.clay")?;
        let types = typechecker::build(parsed)?;
        build(types)
    }
}
