use super::access::Access;
use super::{column_id::ColumnId, relation::PostgresRelation};
use crate::aggregate::AggregateField;
use crate::model::ModelPostgresSystem;
use crate::operation::{AggregateQuery, CollectionQuery, CollectionQueryParameter, PkQuery};
use async_graphql_parser::types::{
    BaseType, FieldDefinition, InputObjectType, InputValueDefinition, ObjectType, Type,
    TypeDefinition, TypeKind,
};
use async_graphql_value::Name;
use core_plugin_interface::core_model::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    type_normalization::{
        default_positioned, default_positioned_name, FieldDefinitionProvider, InputValueProvider,
        TypeDefinitionProvider, TypeModifier,
    },
};
use payas_sql::PhysicalTable;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresType {
    pub name: String,
    pub plural_name: String,
    pub kind: PostgresTypeKind,
    pub is_input: bool, // Is this to be used as an input field (such as an argument in a mutation)? Needed for introspection
}

impl PostgresType {
    pub fn model_fields(&self) -> Vec<&PostgresField> {
        match &self.kind {
            PostgresTypeKind::Primitive => vec![],
            PostgresTypeKind::Composite(PostgresCompositeType { fields, .. }) => {
                fields.iter().collect()
            }
        }
    }

    pub fn model_field(&self, name: &str) -> Option<&PostgresField> {
        self.model_fields()
            .into_iter()
            .find(|model_field| model_field.name == name)
    }

    pub fn pk_field(&self) -> Option<&PostgresField> {
        match &self.kind {
            PostgresTypeKind::Primitive => None,
            PostgresTypeKind::Composite(composite_type) => composite_type.pk_field(),
        }
    }

    pub fn pk_column_id(&self) -> Option<ColumnId> {
        match &self.kind {
            PostgresTypeKind::Primitive => None,
            PostgresTypeKind::Composite(composite_type) => composite_type.pk_column_id(),
        }
    }

    pub fn table_id(&self) -> Option<SerializableSlabIndex<PhysicalTable>> {
        match &self.kind {
            PostgresTypeKind::Composite(PostgresCompositeType { table_id, .. }) => Some(*table_id),
            _ => None,
        }
    }

    pub fn is_primitive(&self) -> bool {
        matches!(&self.kind, PostgresTypeKind::Primitive)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PostgresTypeKind {
    Primitive,
    Composite(PostgresCompositeType),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostgresCompositeType {
    pub fields: Vec<PostgresField>,
    pub agg_fields: Vec<AggregateField>,
    pub table_id: SerializableSlabIndex<PhysicalTable>,
    pub pk_query: SerializableSlabIndex<PkQuery>,
    pub collection_query: SerializableSlabIndex<CollectionQuery>,
    pub aggregate_query: SerializableSlabIndex<AggregateQuery>,
    pub access: Access,
}

impl PostgresCompositeType {
    pub fn get_field_by_name(&self, name: &str) -> Option<&PostgresField> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub fn pk_field(&self) -> Option<&PostgresField> {
        self.fields
            .iter()
            .find(|field| matches!(&field.relation, PostgresRelation::Pk { .. }))
    }

    pub fn pk_column_id(&self) -> Option<ColumnId> {
        self.pk_field()
            .and_then(|pk_field| pk_field.relation.self_column())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PostgresTypeModifier {
    Optional,
    NonNull,
    List,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostgresField {
    pub name: String,
    pub typ: PostgresFieldType,
    pub relation: PostgresRelation,
    pub has_default_value: bool, // does this field have a default value?
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PostgresFieldType {
    Optional(Box<PostgresFieldType>),
    Reference {
        type_id: SerializableSlabIndex<PostgresType>,
        type_name: String,
    },
    List(Box<PostgresFieldType>),
}

impl PostgresFieldType {
    pub fn type_id(&self) -> &SerializableSlabIndex<PostgresType> {
        match self {
            PostgresFieldType::Optional(underlying) | PostgresFieldType::List(underlying) => {
                underlying.type_id()
            }
            PostgresFieldType::Reference { type_id, .. } => type_id,
        }
    }

    pub fn base_type<'a>(&self, types: &'a SerializableSlab<PostgresType>) -> &'a PostgresType {
        match self {
            PostgresFieldType::Optional(underlying) | PostgresFieldType::List(underlying) => {
                underlying.base_type(types)
            }
            PostgresFieldType::Reference { type_id, .. } => &types[*type_id],
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            PostgresFieldType::Optional(underlying) | PostgresFieldType::List(underlying) => {
                underlying.type_name()
            }
            PostgresFieldType::Reference { type_name, .. } => type_name,
        }
    }

    pub fn optional(&self) -> Self {
        match self {
            PostgresFieldType::Optional(_) => self.clone(),
            _ => PostgresFieldType::Optional(Box::new(self.clone())),
        }
    }
}

impl TypeDefinitionProvider<ModelPostgresSystem> for PostgresType {
    fn type_definition(&self, system: &ModelPostgresSystem) -> TypeDefinition {
        match &self.kind {
            PostgresTypeKind::Primitive => TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(&self.name),
                directives: vec![],
                kind: TypeKind::Scalar,
            },
            PostgresTypeKind::Composite(PostgresCompositeType {
                fields: model_fields,
                agg_fields,
                ..
            }) => {
                let kind = if self.is_input {
                    let fields = model_fields
                        .iter()
                        .map(|model_field| default_positioned(model_field.input_value()))
                        .collect();
                    TypeKind::InputObject(InputObjectType { fields })
                } else {
                    let model_fields = model_fields.iter().map(|model_field| {
                        default_positioned(model_field.field_definition(system))
                    });

                    let agg_fields = agg_fields
                        .iter()
                        .map(|agg_field| default_positioned(agg_field.field_definition(system)));

                    let fields = model_fields.chain(agg_fields).collect();

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
    }
}

impl FieldDefinitionProvider<ModelPostgresSystem> for PostgresField {
    fn field_definition(&self, system: &ModelPostgresSystem) -> FieldDefinition {
        let field_type = default_positioned(compute_type(&self.typ));

        let arguments = match self.relation {
            PostgresRelation::Pk { .. }
            | PostgresRelation::Scalar { .. }
            | PostgresRelation::ManyToOne { .. } => {
                vec![]
            }
            PostgresRelation::OneToMany { other_type_id, .. } => {
                let other_type = &system.postgres_types[other_type_id];
                match &other_type.kind {
                    PostgresTypeKind::Primitive => panic!(),
                    PostgresTypeKind::Composite(kind) => {
                        let collection_query = &system.collection_queries[kind.collection_query];

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
                }
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

pub fn compute_type(typ: &PostgresFieldType) -> Type {
    fn compute_base_type(typ: &PostgresFieldType) -> BaseType {
        match typ {
            PostgresFieldType::Optional(underlying) => compute_base_type(underlying),
            PostgresFieldType::Reference { type_name, .. } => BaseType::Named(Name::new(type_name)),
            PostgresFieldType::List(underlying) => {
                BaseType::List(Box::new(compute_type(underlying)))
            }
        }
    }

    match typ {
        PostgresFieldType::Optional(underlying) => Type {
            base: compute_base_type(underlying),
            nullable: true,
        },
        PostgresFieldType::Reference { type_name, .. } => Type {
            base: BaseType::Named(Name::new(type_name)),
            nullable: false,
        },
        PostgresFieldType::List(underlying) => Type {
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

// We need to a special case for the GqlField type, so that we can properly
// created nested types such as Optional(List(List(String))). The blanket impl
// above will not work for nested types like these.
impl InputValueProvider for PostgresField {
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
