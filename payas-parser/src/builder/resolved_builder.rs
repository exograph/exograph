//! Resolve types to consume and normalize annotations.
//!
//! For example, while in `Type`, the fields carry an optional @column annotation for the
//! column name, here that information is encoded into an attribute of `ResolvedType`.
//! If no @column is provided, the encoded information is set to an appropriate default value.

use anyhow::Result;

use payas_model::model::mapped_arena::MappedArena;
use payas_model::model::naming::{ToPlural, ToTableName};

use crate::ast::ast_types::{AstAnnotationParams, AstFieldType};
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
    pub table_name: String,
    pub access: ResolvedAccess,
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
    pub column_name: String,
    pub is_pk: bool,
    pub is_autoincrement: bool,
    pub type_hint: Option<ResolvedTypeHint>,
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
        self.fields.iter().find(|f| f.is_pk)
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
    let mut resolved_system = build_shallow(&types)?;
    build_expanded(types, &mut resolved_system);
    Ok(resolved_system)
}

fn build_shallow(types: &MappedArena<Type>) -> Result<ResolvedSystem> {
    let mut resolved_types: MappedArena<ResolvedType> = MappedArena::default();
    let mut resolved_contexts: MappedArena<ResolvedContext> = MappedArena::default();

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
                        table_name,
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
            o => panic!(
                "Unable to build shallow type for non-primitve, non-composite type: {:?}",
                o
            ),
        };
    }

    Ok(ResolvedSystem {
        types: resolved_types,
        contexts: resolved_contexts,
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

fn build_expanded(types: MappedArena<Type>, resolved_system: &mut ResolvedSystem) {
    for (_, typ) in types.iter() {
        if let Type::Composite(ct) = typ {
            if ct.kind == AstModelKind::Persistent {
                build_expanded_persistent_type(ct, &types, resolved_system);
            } else {
                build_expanded_context_type(ct, &types, resolved_system);
            }
        }
    }
}

fn build_expanded_persistent_type(
    ct: &AstModel<Typed>,
    types: &MappedArena<Type>,
    resolved_system: &mut ResolvedSystem,
) {
    let resolved_types = &mut resolved_system.types;

    let existing_type_id = resolved_types.get_id(&ct.name).unwrap();
    let existing_type = &resolved_types[existing_type_id];

    if let ResolvedType::Composite(ResolvedCompositeType {
        name,
        plural_name,
        table_name,
        access,
        ..
    }) = existing_type
    {
        let resolved_fields = ct
            .fields
            .iter()
            .map(|field| ResolvedField {
                name: field.name.clone(),
                typ: resolve_field_type(&field.typ.to_typ(types), types, resolved_types),
                column_name: compute_column_name(ct, field, types),
                is_pk: field.annotations.contains("pk"),
                is_autoincrement: field.annotations.contains("autoincrement"),
                type_hint: build_type_hint(field, types),
            })
            .collect();

        let expanded = ResolvedType::Composite(ResolvedCompositeType {
            name: name.clone(),
            plural_name: plural_name.clone(),
            fields: resolved_fields,
            table_name: table_name.clone(),
            access: access.clone(),
        });
        resolved_types[existing_type_id] = expanded;
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
) -> String {
    fn default_column_name(
        enclosing_type: &AstModel<Typed>,
        field: &AstField<Typed>,
        types: &MappedArena<Type>,
    ) -> String {
        match &field.typ {
            AstFieldType::Optional(_) => field.name.to_string(),
            AstFieldType::Plain(_, _, _, _) => {
                let field_type = field.typ.to_typ(types).deref(types);
                match field_type {
                    Type::Composite(_) => format!("{}_id", field.name),
                    Type::Set(typ) => {
                        if let Type::Composite(_) = typ.deref(types) {
                            // OneToMany
                            format!("{}_id", enclosing_type.name.to_ascii_lowercase())
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
                            field.name.clone()
                        } else {
                            panic!("Arrays of non-primitives are not supported");
                        }
                    }

                    _ => field.name.clone(),
                }
            }
        }
    }

    field
        .annotations
        .get("column")
        .map(|p| p.as_single().as_string())
        .unwrap_or_else(|| default_column_name(enclosing_type, field, types))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parser, typechecker};

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
          concerts: Set[Concert] @column("custom_venueid")
          capacity: Int @bits(16)
          latitude: Float @size(4)
        }        
        "#;

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
          attending: Array[String]
          seating: Array[Array[Boolean]]
        }

        model Venue             {
          id: Int  @autoincrement @pk 
          name:String 
          concerts: Set[Concert] 
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
        "#;

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

    fn create_resolved_system(src: &str) -> ResolvedSystem {
        let (parsed, codemap) = parser::parse_str(src);
        let types = typechecker::build(parsed, codemap).unwrap();

        build(types).unwrap()
    }
}
