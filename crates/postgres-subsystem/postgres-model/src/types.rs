use super::access::Access;
use super::{column_id::ColumnId, relation::PostgresRelation};
use crate::aggregate::AggregateField;
use crate::operation::{AggregateQuery, CollectionQuery, CollectionQueryParameter, PkQuery};
use crate::subsystem::PostgresSubsystem;
use async_graphql_parser::types::{
    BaseType, FieldDefinition, InputObjectType, InputValueDefinition, ObjectType, Type,
    TypeDefinition, TypeKind,
};
use async_graphql_value::Name;
use core_plugin_interface::core_model::types::Named;
use core_plugin_interface::core_model::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    type_normalization::{
        default_positioned, default_positioned_name, FieldDefinitionProvider, InputValueProvider,
        TypeDefinitionProvider, TypeModifier,
    },
};
use payas_sql::PhysicalTable;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum TypeIndex<CT> {
    Primitive(SerializableSlabIndex<PostgresPrimitiveType>),
    Composite(SerializableSlabIndex<CT>),
}

impl<CT> TypeIndex<CT> {
    pub fn to_type<'a>(
        &self,
        primitive_types: &'a SerializableSlab<PostgresPrimitiveType>,
        entity_types: &'a SerializableSlab<CT>,
    ) -> PostgresType<'a, CT> {
        match self {
            TypeIndex::Primitive(index) => PostgresType::Primitive(&primitive_types[*index]),
            TypeIndex::Composite(index) => PostgresType::Composite(&entity_types[*index]),
        }
    }
}

#[derive(Debug)]
pub enum PostgresType<'a, CT> {
    Primitive(&'a PostgresPrimitiveType),
    Composite(&'a CT),
}

impl<'a, CT: Named> PostgresType<'a, CT> {
    pub fn name(&self) -> &str {
        match self {
            PostgresType::Primitive(primitive_type) => &primitive_type.name,
            PostgresType::Composite(composite_type) => composite_type.name(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresPrimitiveType {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityType {
    pub name: String,
    pub plural_name: String,

    pub fields: Vec<PostgresField<EntityType>>,
    pub agg_fields: Vec<AggregateField>,
    pub table_id: SerializableSlabIndex<PhysicalTable>,
    pub pk_query: SerializableSlabIndex<PkQuery>,
    pub collection_query: SerializableSlabIndex<CollectionQuery>,
    pub aggregate_query: SerializableSlabIndex<AggregateQuery>,
    pub access: Access,
}

impl Named for EntityType {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Mutation input types such as `CreatePostInput` and `UpdatePostInput`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MutationType {
    pub name: String,
    pub fields: Vec<PostgresField<MutationType>>,
    pub entity_type: SerializableSlabIndex<EntityType>,
}

impl EntityType {
    pub fn field(&self, name: &str) -> Option<&PostgresField<EntityType>> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub fn pk_field(&self) -> Option<&PostgresField<EntityType>> {
        self.fields
            .iter()
            .find(|field| matches!(&field.relation, PostgresRelation::Pk { .. }))
    }

    pub fn pk_column_id(&self) -> Option<ColumnId> {
        self.pk_field()
            .and_then(|pk_field| pk_field.relation.self_column())
    }

    pub fn aggregate_field(&self, name: &str) -> Option<&AggregateField> {
        self.agg_fields.iter().find(|field| field.name == name)
    }
}

impl MutationType {
    pub fn table<'a>(&'a self, system: &'a PostgresSubsystem) -> &'a PhysicalTable {
        let entity_type = &system.entity_types[self.entity_type];
        let table_id = entity_type.table_id;
        &system.tables[table_id]
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PostgresTypeModifier {
    Optional,
    NonNull,
    List,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostgresField<CT> {
    pub name: String,
    pub typ: FieldType<CT>,
    pub relation: PostgresRelation,
    pub has_default_value: bool, // does this field have a default value?
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FieldType<CT> {
    Optional(Box<FieldType<CT>>),
    Reference {
        type_id: TypeIndex<CT>,
        type_name: String,
    },
    List(Box<FieldType<CT>>),
}

impl<CT> FieldType<CT>
where
    CT: Clone,
{
    pub fn type_id(&self) -> &TypeIndex<CT> {
        match self {
            FieldType::Optional(underlying) | FieldType::List(underlying) => underlying.type_id(),
            FieldType::Reference { type_id, .. } => type_id,
        }
    }

    pub fn base_type<'a>(
        &self,
        primitive_types: &'a SerializableSlab<PostgresPrimitiveType>,
        entity_types: &'a SerializableSlab<CT>,
    ) -> PostgresType<'a, CT> {
        match self {
            FieldType::Optional(underlying) | FieldType::List(underlying) => {
                underlying.base_type(primitive_types, entity_types)
            }
            FieldType::Reference { type_id, .. } => type_id.to_type(primitive_types, entity_types),
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            FieldType::Optional(underlying) | FieldType::List(underlying) => underlying.type_name(),
            FieldType::Reference { type_name, .. } => type_name,
        }
    }

    pub fn optional(&self) -> Self {
        match self {
            FieldType::Optional(_) => self.clone(),
            _ => FieldType::Optional(Box::new(self.clone())),
        }
    }
}

impl TypeDefinitionProvider<PostgresSubsystem> for PostgresPrimitiveType {
    fn type_definition(&self, _system: &PostgresSubsystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}

impl TypeDefinitionProvider<PostgresSubsystem> for EntityType {
    fn type_definition(&self, system: &PostgresSubsystem) -> TypeDefinition {
        let EntityType {
            fields, agg_fields, ..
        } = self;

        let kind = {
            let entity = fields
                .iter()
                .map(|field| default_positioned(field.field_definition(system)));

            let agg_fields = agg_fields
                .iter()
                .map(|field| default_positioned(field.field_definition(system)));

            let fields = entity.chain(agg_fields).collect();

            TypeKind::Object(ObjectType {
                implements: vec![],
                fields,
            })
        };
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind,
        }
    }
}

impl TypeDefinitionProvider<PostgresSubsystem> for MutationType {
    fn type_definition(&self, _system: &PostgresSubsystem) -> TypeDefinition {
        let kind = {
            let fields = self
                .fields
                .iter()
                .map(|field| default_positioned(field.input_value()))
                .collect();
            TypeKind::InputObject(InputObjectType { fields })
        };
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind,
        }
    }
}

impl<CT> FieldDefinitionProvider<PostgresSubsystem> for PostgresField<CT> {
    fn field_definition(&self, system: &PostgresSubsystem) -> FieldDefinition {
        let field_type = default_positioned(compute_type(&self.typ));

        let arguments = match self.relation {
            PostgresRelation::Pk { .. }
            | PostgresRelation::Scalar { .. }
            | PostgresRelation::ManyToOne { .. } => {
                vec![]
            }
            PostgresRelation::OneToMany { other_type_id, .. } => {
                let other_type = &system.entity_types[other_type_id];
                let collection_query = &system.collection_queries[other_type.collection_query];

                let CollectionQueryParameter {
                    predicate_param,
                    order_by_param,
                    limit_param,
                    offset_param,
                } = &collection_query.parameter;

                [
                    predicate_param.input_value(),
                    order_by_param.input_value(),
                    limit_param.input_value(),
                    offset_param.input_value(),
                ]
                .into_iter()
                .map(default_positioned)
                .collect()
            }
        };

        FieldDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            arguments,
            ty: field_type,
            directives: vec![],
        }
    }
}

pub fn compute_type<CT>(typ: &FieldType<CT>) -> Type {
    fn compute_base_type<CT>(typ: &FieldType<CT>) -> BaseType {
        match typ {
            FieldType::Optional(underlying) => compute_base_type(underlying),
            FieldType::Reference { type_name, .. } => BaseType::Named(Name::new(type_name)),
            FieldType::List(underlying) => BaseType::List(Box::new(compute_type(underlying))),
        }
    }

    match typ {
        FieldType::Optional(underlying) => Type {
            base: compute_base_type(underlying),
            nullable: true,
        },
        FieldType::Reference { type_name, .. } => Type {
            base: BaseType::Named(Name::new(type_name)),
            nullable: false,
        },
        FieldType::List(underlying) => Type {
            base: BaseType::List(Box::new(compute_type(underlying))),
            nullable: false,
        },
    }
}

impl From<&PostgresTypeModifier> for TypeModifier {
    fn from(modifier: &PostgresTypeModifier) -> Self {
        match modifier {
            PostgresTypeModifier::Optional => TypeModifier::Optional,
            PostgresTypeModifier::NonNull => TypeModifier::NonNull,
            PostgresTypeModifier::List => TypeModifier::List,
        }
    }
}

// We need to a special case for the PostgresField type, so that we can properly
// created nested types such as Optional(List(List(String))). The blanket impl
// above will not work for nested types like these.
impl<CT> InputValueProvider for PostgresField<CT> {
    fn input_value(&self) -> InputValueDefinition {
        let field_type = default_positioned(compute_type(&self.typ));

        InputValueDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            ty: field_type,
            default_value: None,
            directives: vec![],
        }
    }
}
