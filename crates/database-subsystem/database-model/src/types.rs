use super::access::Access;
use super::{column_id::ColumnId, relation::DatabaseRelation};
use crate::model::ModelDatabaseSystem;
use crate::operation::{DatabaseQuery, DatabaseQueryParameter};
use async_graphql_parser::types::{
    BaseType, FieldDefinition, InputObjectType, InputValueDefinition, ObjectType, Type,
    TypeDefinition, TypeKind,
};
use async_graphql_value::Name;
use core_model::mapped_arena::{SerializableSlab, SerializableSlabIndex};

use core_model::type_normalization::{
    default_positioned, default_positioned_name, FieldDefinitionProvider, InputValueProvider,
    TypeDefinitionProvider, TypeModifier,
};
use payas_sql::PhysicalTable;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DatabaseType {
    pub name: String,
    pub plural_name: String,
    pub kind: DatabaseTypeKind,
    pub is_input: bool, // Is this to be used as an input field (such as an argument in a mutation)? Needed for introspection
}

impl DatabaseType {
    pub fn model_fields(&self) -> Vec<&DatabaseField> {
        match &self.kind {
            DatabaseTypeKind::Primitive => vec![],
            DatabaseTypeKind::Composite(DatabaseCompositeType { fields, .. }) => {
                fields.iter().collect()
            }
        }
    }

    pub fn model_field(&self, name: &str) -> Option<&DatabaseField> {
        self.model_fields()
            .into_iter()
            .find(|model_field| model_field.name == name)
    }

    pub fn pk_field(&self) -> Option<&DatabaseField> {
        match &self.kind {
            DatabaseTypeKind::Primitive => None,
            DatabaseTypeKind::Composite(composite_type) => composite_type.pk_field(),
        }
    }

    pub fn pk_column_id(&self) -> Option<ColumnId> {
        match &self.kind {
            DatabaseTypeKind::Primitive => None,
            DatabaseTypeKind::Composite(composite_type) => composite_type.pk_column_id(),
        }
    }

    pub fn table_id(&self) -> Option<SerializableSlabIndex<PhysicalTable>> {
        match &self.kind {
            DatabaseTypeKind::Composite(DatabaseCompositeType { table_id, .. }) => Some(*table_id),
            _ => None,
        }
    }

    pub fn is_primitive(&self) -> bool {
        matches!(&self.kind, DatabaseTypeKind::Primitive)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum DatabaseTypeKind {
    Primitive,
    Composite(DatabaseCompositeType),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DatabaseCompositeType {
    pub fields: Vec<DatabaseField>,
    pub table_id: SerializableSlabIndex<PhysicalTable>,
    pub pk_query: SerializableSlabIndex<DatabaseQuery>,
    pub collection_query: SerializableSlabIndex<DatabaseQuery>,
    pub access: Access,
}

impl DatabaseCompositeType {
    pub fn get_field_by_name(&self, name: &str) -> Option<&DatabaseField> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub fn pk_field(&self) -> Option<&DatabaseField> {
        self.fields
            .iter()
            .find(|field| matches!(&field.relation, DatabaseRelation::Pk { .. }))
    }

    pub fn pk_column_id(&self) -> Option<ColumnId> {
        self.pk_field()
            .and_then(|pk_field| pk_field.relation.self_column())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DatabaseTypeModifier {
    Optional,
    NonNull,
    List,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DatabaseField {
    pub name: String,
    pub typ: DatabaseFieldType,
    pub relation: DatabaseRelation,
    pub has_default_value: bool, // does this field have a default value?
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DatabaseFieldType {
    Optional(Box<DatabaseFieldType>),
    Reference {
        type_id: SerializableSlabIndex<DatabaseType>,
        is_primitive: bool, // A way to know which arena to look up the type in
        type_name: String,
    },
    List(Box<DatabaseFieldType>),
}

impl DatabaseFieldType {
    pub fn type_id(&self) -> &SerializableSlabIndex<DatabaseType> {
        match self {
            DatabaseFieldType::Optional(underlying) | DatabaseFieldType::List(underlying) => {
                underlying.type_id()
            }
            DatabaseFieldType::Reference { type_id, .. } => type_id,
        }
    }

    pub fn is_primitive(&self) -> bool {
        match self {
            DatabaseFieldType::Optional(underlying) | DatabaseFieldType::List(underlying) => {
                underlying.is_primitive()
            }
            DatabaseFieldType::Reference { is_primitive, .. } => *is_primitive,
        }
    }

    pub fn base_type<'a>(&self, types: &'a SerializableSlab<DatabaseType>) -> &'a DatabaseType {
        match self {
            DatabaseFieldType::Optional(underlying) | DatabaseFieldType::List(underlying) => {
                underlying.base_type(types)
            }
            DatabaseFieldType::Reference { type_id, .. } => &types[*type_id],
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            DatabaseFieldType::Optional(underlying) | DatabaseFieldType::List(underlying) => {
                underlying.type_name()
            }
            DatabaseFieldType::Reference { type_name, .. } => type_name,
        }
    }

    pub fn optional(&self) -> Self {
        match self {
            DatabaseFieldType::Optional(_) => self.clone(),
            _ => DatabaseFieldType::Optional(Box::new(self.clone())),
        }
    }
}

impl TypeDefinitionProvider<ModelDatabaseSystem> for DatabaseType {
    fn type_definition(&self, system: &ModelDatabaseSystem) -> TypeDefinition {
        match &self.kind {
            DatabaseTypeKind::Primitive => TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(&self.name),
                directives: vec![],
                kind: TypeKind::Scalar,
            },
            DatabaseTypeKind::Composite(DatabaseCompositeType {
                fields: model_fields,
                ..
            }) => {
                let kind = if self.is_input {
                    let fields = model_fields
                        .iter()
                        .map(|model_field| default_positioned(model_field.input_value()))
                        .collect();
                    TypeKind::InputObject(InputObjectType { fields })
                } else {
                    let fields: Vec<_> = model_fields
                        .iter()
                        .map(|model_field| default_positioned(model_field.field_definition(system)))
                        .collect();

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

impl FieldDefinitionProvider<ModelDatabaseSystem> for DatabaseField {
    fn field_definition(&self, system: &ModelDatabaseSystem) -> FieldDefinition {
        let field_type = default_positioned(compute_type(&self.typ));

        let arguments = match self.relation {
            DatabaseRelation::Pk { .. }
            | DatabaseRelation::Scalar { .. }
            | DatabaseRelation::ManyToOne { .. } => {
                vec![]
            }
            DatabaseRelation::OneToMany { other_type_id, .. } => {
                let other_type = &system.database_types[other_type_id];
                match &other_type.kind {
                    DatabaseTypeKind::Primitive => panic!(),
                    DatabaseTypeKind::Composite(kind) => {
                        let collection_query = kind.collection_query;
                        let collection_query = &system.queries[collection_query];

                        let DatabaseQueryParameter {
                            predicate_param,
                            order_by_param,
                            limit_param,
                            offset_param,
                        } = &collection_query.parameter;

                        let predicate_parameter_arg =
                            predicate_param.as_ref().map(|p| p.input_value());
                        let order_by_parameter_arg =
                            order_by_param.as_ref().map(|p| p.input_value());
                        let limit_arg = limit_param.as_ref().map(|p| p.input_value());
                        let offset_arg = offset_param.as_ref().map(|p| p.input_value());

                        vec![
                            predicate_parameter_arg,
                            order_by_parameter_arg,
                            limit_arg,
                            offset_arg,
                        ]
                        .into_iter()
                        .flatten()
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

pub fn compute_type(typ: &DatabaseFieldType) -> Type {
    fn compute_base_type(typ: &DatabaseFieldType) -> BaseType {
        match typ {
            DatabaseFieldType::Optional(underlying) => compute_base_type(underlying),
            DatabaseFieldType::Reference { type_name, .. } => BaseType::Named(Name::new(type_name)),
            DatabaseFieldType::List(underlying) => {
                BaseType::List(Box::new(compute_type(underlying)))
            }
        }
    }

    match typ {
        DatabaseFieldType::Optional(underlying) => Type {
            base: compute_base_type(underlying),
            nullable: true,
        },
        DatabaseFieldType::Reference { type_name, .. } => Type {
            base: BaseType::Named(Name::new(type_name)),
            nullable: false,
        },
        DatabaseFieldType::List(underlying) => Type {
            base: BaseType::List(Box::new(compute_type(underlying))),
            nullable: false,
        },
    }
}

impl From<&DatabaseTypeModifier> for TypeModifier {
    fn from(modifier: &DatabaseTypeModifier) -> Self {
        match modifier {
            DatabaseTypeModifier::Optional => TypeModifier::Optional,
            DatabaseTypeModifier::NonNull => TypeModifier::NonNull,
            DatabaseTypeModifier::List => TypeModifier::List,
        }
    }
}

// We need to a special case for the GqlField type, so that we can properly
// created nested types such as Optional(List(List(String))). The blanket impl
// above will not work for nested types like these.
impl InputValueProvider for DatabaseField {
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
