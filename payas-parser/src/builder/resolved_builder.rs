use payas_model::model::mapped_arena::MappedArena;

use crate::typechecker::{
    CompositeType, CompositeTypeKind, PrimitiveType, Type, TypedExpression, TypedField,
    TypedFieldSelection,
};
use serde::{Deserialize, Serialize};

pub struct ResolvedSystem {
    pub types: MappedArena<ResolvedType>,
    pub contexts: MappedArena<ResolvedContext>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolvedType {
    Primitive(PrimitiveType),
    Composite(ResolvedCompositeType),
}

pub struct ResolvedContext {
    pub name: String,
    pub fields: Vec<ResolvedContextField>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedCompositeType {
    pub name: String,
    pub fields: Vec<ResolvedField>,
    pub table_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedField {
    pub name: String,
    pub typ: ResolvedFieldType,
    pub column_name: String,
    pub is_pk: bool,
    pub is_autoincrement: bool,
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
    pub fn as_composite<'a>(&'a self) -> &'a ResolvedCompositeType {
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
/// Resolved types normalized annotations in that if an annotaion isn't provided,
/// this build compute the default value and sets that information in the resulting resolved type
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
                    .get_annotation("table")
                    .map(|a| a.params[0].as_string())
                    .unwrap_or_else(|| ct.name.clone());
                resolved_types.add(
                    &ct.name,
                    ResolvedType::Composite(ResolvedCompositeType {
                        name: ct.name.clone(),
                        fields: vec![],
                        table_name,
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
        name, table_name, ..
    }) = existing_type
    {
        let resolved_fields = ct
            .fields
            .iter()
            .map(|field| ResolvedField {
                name: field.name.clone(),
                typ: resolve_field_type(&field.typ, &types, resolved_types),
                column_name: compute_column_name(ct, field, &types),
                is_pk: field.get_annotation("pk").is_some(),
                is_autoincrement: field.get_annotation("autoincrement").is_some(),
            })
            .collect();

        let expanded = ResolvedType::Composite(ResolvedCompositeType {
            name: name.clone(),
            fields: resolved_fields,
            table_name: table_name.clone(),
        });
        resolved_types[existing_type_id] = expanded;
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
        .map(|field| {
            let jwt_annot = field
                .annotations
                .iter()
                .find(|annotation| annotation.name == "jwt");
            let claim = jwt_annot
                .map(|annot| match &annot.params[0] {
                    TypedExpression::FieldSelection(selection) => match selection {
                        TypedFieldSelection::Single(claim, _) => claim.0.clone(),
                        _ => panic!("Only simple jwt claim supported"),
                    },
                    _ => panic!("Expression type other than selection unsupported"),
                })
                .unwrap();

            ResolvedContextField {
                name: field.name.clone(),
                typ: resolve_field_type(&field.typ, &types, resolved_types),
                source: ResolvedContextSource::Jwt { claim },
            }
        })
        .collect();

    let expanded = ResolvedContext {
        name: existing_type.name.clone(),
        fields: resolved_fields,
    };
    resolved_contexts[existing_type_id] = expanded;
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
        .get_annotation("column")
        .map(|a| a.params[0].as_string())
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
          id: Int @pk @autoincrement @column("custom_id")
          title: String @column("custom_title")
          venue: Venue @column("custom_venue_id")
        }
        
        @table("venues")
        model Venue {
          id: Int @pk @autoincrement @column("custom_id")
          name: String @column("custom_name")
          concerts: [Concert] @column("custom_venueid")
        }        
        "#;

        let (parsed, codemap) = parser::parse_str(src);
        let types = typechecker::build(parsed, codemap);

        let resolved = build(types);

        insta::assert_yaml_snapshot!(normalized_system(&resolved).0);
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

        let (parsed, codemap) = parser::parse_str(src);
        let types = typechecker::build(parsed, codemap);

        let resolved = build(types);

        insta::assert_yaml_snapshot!(normalized_system(&resolved).0);
    }

    fn normalized_system<'a>(
        input: &'a ResolvedSystem,
    ) -> (Vec<&'a ResolvedType>, Vec<&'a ResolvedContext>) {
        (
            sorted_values(&input.types).clone(),
            sorted_values(&input.contexts),
        )
    }
}
