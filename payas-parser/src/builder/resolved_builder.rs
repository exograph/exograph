use payas_model::model::mapped_arena::MappedArena;

use crate::typechecker::{
    AccessAnnotation, CompositeType, CompositeTypeKind, PrimitiveType, RangeAnnotation, Type,
    TypedExpression, TypedField, TypedFieldSelection,
};
use serde::{Deserialize, Serialize};

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
    pub creation: TypedExpression,
    pub read: TypedExpression,
    pub update: TypedExpression,
    pub delete: TypedExpression,
}

impl ResolvedAccess {
    fn permissive() -> Self {
        ResolvedAccess {
            creation: TypedExpression::BooleanLiteral(
                true,
                Type::Primitive(PrimitiveType::Boolean),
            ),
            read: TypedExpression::BooleanLiteral(true, Type::Primitive(PrimitiveType::Boolean)),
            update: TypedExpression::BooleanLiteral(true, Type::Primitive(PrimitiveType::Boolean)),
            delete: TypedExpression::BooleanLiteral(true, Type::Primitive(PrimitiveType::Boolean)),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedCompositeType {
    pub name: String,
    pub fields: Vec<ResolvedField>,
    pub table_name: String,
    pub access: ResolvedAccess,
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
    ExplicitHint {
        dbtype: String,
    },
    IntHint {
        bits: Option<usize>,
        range: Option<(i64, i64)>,
    },
    StringHint {
        length: usize,
    },
}

impl ResolvedCompositeType {
    pub fn pk_field(&self) -> Option<&ResolvedField> {
        self.fields.iter().find(|f| f.is_pk)
    }
}

impl ResolvedType {
    fn name(&self) -> &str {
        match self {
            ResolvedType::Primitive(pt) => pt.name(),
            ResolvedType::Composite(ResolvedCompositeType { name, .. }) => name,
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
            ResolvedFieldType::Plain(name) => types.get_by_key(&name).unwrap(),
            ResolvedFieldType::Optional(underlying) | ResolvedFieldType::List(underlying) => {
                underlying.deref(types)
            }
        }
    }
}

/// Consume typed-checked types and build resolved types
/// Resolved types consume and normalize annotations.
/// For example, while in `Type`, the fields carry an optional annotation for the column name, here that information is encoded into an attribute of `ResolvedType`.
/// If an annotaion is missing, its default value is assumed.
pub fn build(types: MappedArena<Type>) -> ResolvedSystem {
    let mut resolved_system = build_shallow(&types);
    build_expanded(types, &mut resolved_system);
    resolved_system
}

fn build_shallow(types: &MappedArena<Type>) -> ResolvedSystem {
    let mut resolved_types: MappedArena<ResolvedType> = MappedArena::default();
    let mut resolved_contexts: MappedArena<ResolvedContext> = MappedArena::default();

    for (_, typ) in types.iter() {
        match typ {
            Type::Primitive(pt) => {
                resolved_types.add(pt.name(), ResolvedType::Primitive(pt.clone()));
            }
            Type::Composite(ct) if ct.kind == CompositeTypeKind::Persistent => {
                let table_name = ct
                    .annotations
                    .table()
                    .map(|a| a.value().as_string())
                    .unwrap_or_else(|| ct.name.clone());
                let access = build_access(ct.annotations.access());
                resolved_types.add(
                    &ct.name,
                    ResolvedType::Composite(ResolvedCompositeType {
                        name: ct.name.clone(),
                        fields: vec![],
                        table_name,
                        access,
                    }),
                );
            }
            Type::Composite(ct) if ct.kind == CompositeTypeKind::Context => {
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

    ResolvedSystem {
        types: resolved_types,
        contexts: resolved_contexts,
    }
}

fn build_access(access_annotation: Option<&AccessAnnotation>) -> ResolvedAccess {
    match access_annotation {
        Some(access) => {
            let restrictive =
                TypedExpression::BooleanLiteral(false, Type::Primitive(PrimitiveType::Boolean));

            // The annotation parameter hierarchy is:
            // value -> query
            //       -> mutation -> create
            //                   -> update
            //                   -> delete
            // Any lower node in the hierarchy get a priority over it parent.

            let (creation, read, update, delete) = match access {
                AccessAnnotation::Single(default) => (default, default, default, default),
                AccessAnnotation::Map {
                    query,
                    mutation,
                    create,
                    update,
                    delete,
                } => {
                    let default_mutation_access = mutation.as_ref().unwrap_or(&restrictive);
                    (
                        create.as_ref().unwrap_or(default_mutation_access),
                        query.as_ref().unwrap_or(&restrictive),
                        update.as_ref().unwrap_or(default_mutation_access),
                        delete.as_ref().unwrap_or(default_mutation_access),
                    )
                }
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
            if ct.kind == CompositeTypeKind::Persistent {
                build_expanded_persistent_type(ct, &types, resolved_system);
            } else {
                build_expanded_context_type(ct, &types, resolved_system);
            }
        }
    }
}

fn build_expanded_persistent_type(
    ct: &CompositeType,
    types: &MappedArena<Type>,
    resolved_system: &mut ResolvedSystem,
) {
    let resolved_types = &mut resolved_system.types;

    let existing_type_id = resolved_types.get_id(&ct.name).unwrap();
    let existing_type = &resolved_types[existing_type_id];

    if let ResolvedType::Composite(ResolvedCompositeType {
        name,
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
                typ: resolve_field_type(&field.typ, &types, resolved_types),
                column_name: compute_column_name(ct, field, &types),
                is_pk: field.annotations.pk().is_some(),
                is_autoincrement: field.annotations.auto_increment().is_some(),
                type_hint: build_type_hint(&field),
            })
            .collect();

        let expanded = ResolvedType::Composite(ResolvedCompositeType {
            name: name.clone(),
            fields: resolved_fields,
            table_name: table_name.clone(),
            access: access.clone(),
        });
        resolved_types[existing_type_id] = expanded;
    }
}

fn build_type_hint(field: &TypedField) -> Option<ResolvedTypeHint> {
    ////
    // Part 1: parse out and validate hints for each primitive
    ////

    let int_hint = {
        let size_annotation = field
            .annotations
            .size()
            .map(|a| a.value().as_number() as usize);

        let bits_annotation = field
            .annotations
            .bits()
            .map(|a| a.value().as_number() as usize);

        if size_annotation.is_some() && bits_annotation.is_some() {
            panic!("Cannot have both @size and @bits for {}", field.name)
        }

        let range_hint = field
            .annotations
            .range()
            .map(|range_annotation| match range_annotation {
                RangeAnnotation::Map { min, max } => (min.as_number(), max.as_number()),
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
            Some(ResolvedTypeHint::IntHint {
                bits: bits_hint,
                range: range_hint,
            })
        } else {
            // no useful hints to pass along
            None
        }
    };

    let string_hint = {
        let length_annotation = field
            .annotations
            .length()
            .map(|a| a.value().as_number() as usize);

        // None if there is no length annotation
        length_annotation.map(|length| ResolvedTypeHint::StringHint { length })
    };

    let primitive_hints = vec![int_hint, string_hint];

    let explicit_dbtype_hint = field
        .annotations
        .db_type()
        .map(|a| a.value().as_string())
        .map(|s| ResolvedTypeHint::ExplicitHint {
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
    ct: &CompositeType,
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
            typ: resolve_field_type(&field.typ, &types, resolved_types),
            source: extract_context_source(field),
        })
        .collect();

    let expanded = ResolvedContext {
        name: existing_type.name.clone(),
        fields: resolved_fields,
    };
    resolved_contexts[existing_type_id] = expanded;
}

fn extract_context_source(field: &TypedField) -> ResolvedContextSource {
    let jwt_annot = field.annotations.jwt();
    let claim = jwt_annot
        .map(|annot| {
            let annot_param = &annot.value();

            match annot_param {
                Some(TypedExpression::FieldSelection(selection)) => match selection {
                    TypedFieldSelection::Single(claim, _) => claim.0.clone(),
                    _ => panic!("Only simple jwt claim supported"),
                },
                Some(TypedExpression::StringLiteral(name, _)) => name.clone(),
                None => field.name.clone(),
                _ => panic!("Expression type other than selection unsupported"),
            }
        })
        .unwrap();

    ResolvedContextSource::Jwt { claim }
}

fn compute_column_name(
    enclosing_type: &CompositeType,
    field: &TypedField,
    types: &MappedArena<Type>,
) -> String {
    fn default_column_name(
        enclosing_type: &CompositeType,
        field: &TypedField,
        types: &MappedArena<Type>,
    ) -> String {
        match &field.typ {
            Type::Optional(_) => field.name.to_string(),
            Type::List(_) => format!("{}_id", enclosing_type.name.to_ascii_lowercase()),
            Type::Reference(type_name) => {
                let field_type = types.get_by_key(&type_name).unwrap();
                match field_type {
                    Type::Composite(_) => format!("{}_id", field.name),
                    _ => field.name.clone(),
                }
            }
            _ => panic!(""),
        }
    }

    field
        .annotations
        .column()
        .map(|a| a.value().as_string())
        .unwrap_or_else(|| default_column_name(enclosing_type, field, types))
}

fn resolve_field_type(
    typ: &Type,
    types: &MappedArena<Type>,
    resolved_types: &MappedArena<ResolvedType>,
) -> ResolvedFieldType {
    match typ {
        Type::Optional(underlying) => ResolvedFieldType::Optional(Box::new(resolve_field_type(
            underlying,
            types,
            resolved_types,
        ))),
        Type::List(underlying) => ResolvedFieldType::List(Box::new(resolve_field_type(
            underlying,
            types,
            resolved_types,
        ))),
        Type::Reference(name) => ResolvedFieldType::Plain(name.to_owned()),
        t => panic!("Invalid type {:?}", t),
    }
}

#[cfg(test)]
mod tests {
    use payas_model::model::mapped_arena::sorted_values;

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
        }
        
        @table("venues")
        model Venue {
          id: Int @pk @autoincrement @column("custom_id")
          name: String @column("custom_name")
          concerts: [Concert] @column("custom_venueid")
          capacity: Int @bits(16)
        }        
        "#;

        let resolved = create_resolved_system(src);

        insta::assert_yaml_snapshot!(normalized_system(&resolved));
    }

    #[test]
    fn with_defaults() {
        // Note the swapped order between @pk and @autoincrement to assert that our parsing logic permits any order
        let src = r#"
        model Concert {
          id: Int @pk @autoincrement 
          title: String 
          venue: Venue 
        }
        
        model Venue             {
          id: Int  @autoincrement @pk 
          name:String 
          concerts: [Concert] 
        }        
        "#;

        let resolved = create_resolved_system(src);

        insta::assert_yaml_snapshot!(normalized_system(&resolved));
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

        insta::assert_yaml_snapshot!(normalized_system(&resolved));
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

        insta::assert_yaml_snapshot!(normalized_system(&resolved));
    }

    fn create_resolved_system(src: &str) -> ResolvedSystem {
        let (parsed, codemap) = parser::parse_str(src);
        let types = typechecker::build(parsed, codemap);

        build(types)
    }

    fn normalized_system(input: &ResolvedSystem) -> (Vec<&ResolvedType>, Vec<&ResolvedContext>) {
        (
            sorted_values(&input.types).clone(),
            sorted_values(&input.contexts),
        )
    }
}
