use std::collections::HashMap;

use codemap::Span;
use exo_sql::SchemaObjectName;
use postgres_core_model::types::EntityRepresentation;
use serde::{Deserialize, Serialize};

use core_model::{
    context_type::ContextType,
    function_defn::FunctionDefinition,
    mapped_arena::MappedArena,
    primitive_type::PrimitiveType,
    types::{FieldType, Named, TypeValidation, TypeValidationProvider},
};
use core_model_builder::{
    ast::ast_types::{AstExpr, default_span},
    typechecker::Typed,
};

use crate::{access_builder::ResolvedAccess, naming::ToPlural, resolved_builder::Cardinality};

#[derive(Debug)]
pub struct ResolvedTypeEnv<'a> {
    pub contexts: &'a MappedArena<ContextType>,
    pub resolved_types: MappedArena<ResolvedType>,
    pub function_definitions: &'a MappedArena<FunctionDefinition>,
}

impl ResolvedTypeEnv<'_> {
    pub fn get_by_key(&self, key: &str) -> Option<&ResolvedType> {
        self.resolved_types.get_by_key(key)
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ResolvedType {
    Primitive(PrimitiveType),
    Composite(ResolvedCompositeType),
    Enum(ResolvedEnumType),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedEnumType {
    pub name: String,
    pub fields: Vec<String>,
    pub enum_name: SchemaObjectName,
    pub doc_comments: Option<String>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ResolvedCompositeType {
    pub name: String,
    pub plural_name: String,
    pub representation: EntityRepresentation,

    pub fields: Vec<ResolvedField>,
    pub table_name: SchemaObjectName,
    pub access: ResolvedAccess,
    pub doc_comments: Option<String>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

impl ToPlural for ResolvedCompositeType {
    fn self_name(&self) -> String {
        self.name.clone()
    }

    fn to_plural(&self) -> String {
        self.plural_name.clone()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ResolvedField {
    pub name: String,
    pub typ: FieldType<ResolvedFieldType>,
    pub column_names: Vec<String>, // column names for this field (will be multiple of the field is composite and that composite type has multiple pks)
    pub self_column: bool, // is the column name in the same table or does it point to a column in a different table?
    pub is_pk: bool,
    pub access: ResolvedAccess,
    pub type_hint: Option<SerializableTypeHint>,
    pub unique_constraints: Vec<String>,
    pub indices: Vec<String>,
    pub cardinality: Option<Cardinality>,
    pub default_value: Option<ResolvedFieldDefault>,
    pub update_sync: bool,
    pub readonly: bool,
    pub doc_comments: Option<String>,
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
            Some(ResolvedFieldDefault::AutoIncrement(_))
        )
    }

    // In many cases, the field has a single column name, so provide a way to get it while asserting that it has only one column name
    pub fn column_name(&self) -> &str {
        match &self.column_names[..] {
            [name] => name,
            _ => panic!("Expected a single column name for field {self:?}"),
        }
    }
}

/// Trait for resolved type hints
pub trait ResolvedTypeHint: Send + Sync + std::fmt::Debug + std::any::Any {
    /// Get the hint type name for identification
    fn hint_type_name(&self) -> &'static str;

    /// Serialize the type hint to a serde value
    fn serialize_data(&self) -> serde_json::Value;
}

/// Wrapper for serializable type hints
#[derive(Debug)]
pub struct SerializableTypeHint(pub Box<dyn ResolvedTypeHint>);

impl Serialize for SerializableTypeHint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("TypeHint", 2)?;
        state.serialize_field("type", self.0.hint_type_name())?;
        state.serialize_field("data", &self.0.serialize_data())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SerializableTypeHint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct TypeHintData {
            #[serde(rename = "type")]
            type_name: String,
            data: serde_json::Value,
        }

        let hint_data = TypeHintData::deserialize(deserializer)?;

        let boxed_hint = if hint_data.type_name == "Explicit" {
            // Handle ExplicitTypeHint separately since it's not specific to a primitive type
            let hint: ExplicitTypeHint = serde_json::from_value(hint_data.data).map_err(|e| {
                serde::de::Error::custom(format!("Failed to deserialize ExplicitTypeHint: {}", e))
            })?;
            Box::new(hint) as Box<dyn ResolvedTypeHint>
        } else {
            crate::type_provider::PRIMITIVE_TYPE_PROVIDER_REGISTRY
                .get(hint_data.type_name.as_str())
                .ok_or_else(|| {
                    serde::de::Error::custom(format!(
                        "Unknown type hint type: {}. Available types: {:?}",
                        hint_data.type_name,
                        std::iter::once("Explicit")
                            .chain(
                                crate::type_provider::PRIMITIVE_TYPE_PROVIDER_REGISTRY
                                    .keys()
                                    .cloned()
                            )
                            .collect::<Vec<_>>()
                    ))
                })?
                .deserialize_type_hint(hint_data.data)
                .map_err(serde::de::Error::custom)?
        };

        Ok(SerializableTypeHint(boxed_hint))
    }
}

/// Explicit type hint implementation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplicitTypeHint {
    pub dbtype: String,
}

impl ResolvedTypeHint for ExplicitTypeHint {
    fn hint_type_name(&self) -> &'static str {
        "Explicit"
    }

    fn serialize_data(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

impl TypeValidationProvider for SerializableTypeHint {
    fn get_type_validation(&self) -> Option<TypeValidation> {
        let hint_ref = self.0.as_ref() as &dyn std::any::Any;

        // Check if this is an IntTypeHint
        if let Some(int_hint) = hint_ref.downcast_ref::<crate::type_provider::IntTypeHint>() {
            if let Some(r) = &int_hint.range {
                return Some(TypeValidation::Int {
                    range: r.to_owned(),
                });
            }
        }
        // Check if this is a FloatTypeHint
        else if let Some(float_hint) =
            hint_ref.downcast_ref::<crate::type_provider::FloatTypeHint>()
            && let Some(r) = &float_hint.range
        {
            return Some(TypeValidation::Float {
                range: r.to_owned(),
            });
        }
        None
    }
}

impl ResolvedCompositeType {
    pub fn pk_fields(&self) -> Vec<&ResolvedField> {
        self.fields.iter().filter(|f| f.is_pk).collect()
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
    AutoIncrement(Option<SchemaObjectName>),
}

impl ResolvedType {
    pub fn name(&self) -> String {
        match self {
            ResolvedType::Primitive(pt) => pt.name(),
            ResolvedType::Composite(ResolvedCompositeType { name, .. }) => name.to_owned(),
            ResolvedType::Enum(ResolvedEnumType { name, .. }) => name.to_owned(),
        }
    }

    // TODO: Could this return an Option<String> instead? This would avoid the "".to_string() hack
    pub fn plural_name(&self) -> String {
        match self {
            ResolvedType::Primitive(_) | ResolvedType::Enum(_) => "".to_string(), // unused
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
