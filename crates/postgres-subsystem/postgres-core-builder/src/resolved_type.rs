use std::collections::HashMap;

use codemap::Span;
use exo_sql::{PhysicalTableName, VectorDistanceFunction};
use postgres_core_model::types::EntityRepresentation;
use serde::{Deserialize, Serialize};

use core_plugin_interface::{
    core_model::{
        context_type::ContextType,
        function_defn::FunctionDefinition,
        mapped_arena::MappedArena,
        primitive_type::PrimitiveType,
        types::{FieldType, Named, TypeValidation, TypeValidationProvider},
    },
    core_model_builder::{
        ast::ast_types::{default_span, AstExpr},
        typechecker::Typed,
    },
};

use crate::{access_builder::ResolvedAccess, naming::ToPlural};

#[derive(Debug, Clone)]
pub struct ResolvedTypeEnv<'a> {
    pub contexts: &'a MappedArena<ContextType>,
    pub resolved_types: MappedArena<ResolvedType>,
    pub function_definitions: &'a MappedArena<FunctionDefinition>,
}

impl<'a> ResolvedTypeEnv<'a> {
    pub fn get_by_key(&self, key: &str) -> Option<&ResolvedType> {
        self.resolved_types.get_by_key(key)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ResolvedType {
    Primitive(PrimitiveType),
    Composite(ResolvedCompositeType),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedCompositeType {
    pub name: String,
    pub plural_name: String,
    pub representation: EntityRepresentation,

    pub fields: Vec<ResolvedField>,
    pub table_name: PhysicalTableName,
    pub access: ResolvedAccess,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

impl ToPlural for ResolvedCompositeType {
    fn to_singular(&self) -> String {
        self.name.clone()
    }

    fn to_plural(&self) -> String {
        self.plural_name.clone()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedField {
    pub name: String,
    pub typ: FieldType<ResolvedFieldType>,
    pub column_name: String,
    pub self_column: bool, // is the column name in the same table or does it point to a column in a different table?
    pub is_pk: bool,
    pub access: ResolvedAccess,
    pub type_hint: Option<ResolvedTypeHint>,
    pub unique_constraints: Vec<String>,
    pub indices: Vec<String>,
    pub default_value: Option<ResolvedFieldDefault>,
    pub update_sync: bool,
    pub readonly: bool,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    #[allow(unused)]
    pub span: Span,
}

// TODO: dedup?
impl ResolvedField {
    pub fn get_is_auto_increment(&self) -> bool {
        matches!(
            &self.default_value,
            Some(ResolvedFieldDefault::AutoIncrement)
        )
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
        bits: Option<usize>,
        range: Option<(f64, f64)>,
    },
    Decimal {
        precision: Option<usize>,
        scale: Option<usize>,
    },
    String {
        max_length: usize,
    },
    DateTime {
        precision: usize,
    },
    Vector {
        size: Option<usize>,
        distance_function: Option<VectorDistanceFunction>,
    },
}

impl TypeValidationProvider for ResolvedTypeHint {
    fn get_type_validation(&self) -> Option<TypeValidation> {
        match self {
            ResolvedTypeHint::Int { bits: _, range } => {
                if let Some(r) = range {
                    return Some(TypeValidation::Int {
                        range: r.to_owned(),
                    });
                }
                None
            }
            ResolvedTypeHint::Float { bits: _, range } => {
                if let Some(r) = range {
                    return Some(TypeValidation::Float {
                        range: r.to_owned(),
                    });
                }
                None
            }
            _ => None,
        }
    }
}

impl ResolvedCompositeType {
    pub fn pk_field(&self) -> Option<&ResolvedField> {
        self.fields.iter().find(|f| f.is_pk)
    }

    pub fn field_by_column_name(&self, column_name: &str) -> Option<&ResolvedField> {
        self.fields.iter().find(|f| f.column_name == column_name)
    }

    pub fn unique_constraints(&self) -> HashMap<String, Vec<&ResolvedField>> {
        let mut unique_constraints: HashMap<String, Vec<&ResolvedField>> = HashMap::new();

        for field in self.fields.iter() {
            for unique_constraint in field.unique_constraints.iter() {
                unique_constraints
                    .entry(unique_constraint.clone())
                    .or_default()
                    .push(field);
            }
        }

        unique_constraints
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolvedFieldDefault {
    Value(Box<AstExpr<Typed>>),
    PostgresFunction(String),
    AutoIncrement,
}

impl ResolvedType {
    pub fn name(&self) -> String {
        match self {
            ResolvedType::Primitive(pt) => pt.name(),
            ResolvedType::Composite(ResolvedCompositeType { name, .. }) => name.to_owned(),
        }
    }

    // TODO: Could this return an Option<String> instead? This would avoid the "".to_string() hack
    pub fn plural_name(&self) -> String {
        match self {
            ResolvedType::Primitive(_) => "".to_string(), // unused
            ResolvedType::Composite(ResolvedCompositeType { plural_name, .. }) => {
                plural_name.to_owned()
            }
        }
    }

    // useful for relation creation
    pub fn as_composite(&self) -> &ResolvedCompositeType {
        match &self {
            ResolvedType::Composite(c) => c,
            _ => panic!("Cannot get inner composite of type {self:?}"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResolvedFieldType {
    pub type_name: String, // Should really be Id<ResolvedType>, but using String since the former is not serializable as needed by the insta crate
    pub is_primitive: bool, // We need to know if the type is primitive, so that we can look into the correct arena in ModelSystem
}

impl Named for ResolvedFieldType {
    fn name(&self) -> &str {
        &self.type_name
    }
}

pub trait ResolvedFieldTypeHelper {
    fn deref<'a>(&'a self, env: &'a ResolvedTypeEnv) -> &'a ResolvedType;
    fn deref_subsystem_type<'a>(
        &'a self,
        types: &'a MappedArena<ResolvedType>,
    ) -> Option<&'a ResolvedType>;
}

impl ResolvedFieldTypeHelper for FieldType<ResolvedFieldType> {
    fn deref<'a>(&'a self, env: &'a ResolvedTypeEnv) -> &'a ResolvedType {
        env.get_by_key(&self.innermost().type_name).unwrap()
    }

    fn deref_subsystem_type<'a>(
        &'a self,
        types: &'a MappedArena<ResolvedType>,
    ) -> Option<&'a ResolvedType> {
        types.get_by_key(&self.innermost().type_name)
    }
}
