use super::{column_id::ColumnId, relation::ModelRelation};
use crate::model::operation::*;

use crate::sql::PhysicalTable;

use id_arena::Id;
#[derive(Debug, Clone)]
pub struct ModelType {
    pub name: String,
    pub kind: ModelTypeKind,
    pub is_input: bool, // Is this to be used as an input field (such as an argument in a mutation)? Needed for introspection
}

impl ModelType {
    pub fn model_fields(&self) -> Vec<&ModelField> {
        match &self.kind {
            ModelTypeKind::Primitive => vec![],
            ModelTypeKind::Composite { fields, .. } => fields.iter().collect(),
        }
    }

    pub fn model_field(&self, name: &str) -> Option<&ModelField> {
        self.model_fields()
            .into_iter()
            .find(|model_field| model_field.name == name)
    }

    pub fn pk_field(&self) -> Option<&ModelField> {
        self.model_fields().iter().find_map(|field| {
            if let ModelRelation::Pk { .. } = &field.relation {
                Some(*field)
            } else {
                None
            }
        })
    }

    pub fn pk_column_id(&self) -> Option<ColumnId> {
        self.pk_field()
            .and_then(|pk_field| pk_field.relation.self_column())
    }

    pub fn table_id(&self) -> Option<Id<PhysicalTable>> {
        match &self.kind {
            ModelTypeKind::Primitive => None,
            ModelTypeKind::Composite { table_id, .. } => Some(*table_id),
        }
    }

    pub fn is_primitive(&self) -> bool {
        matches!(&self.kind, ModelTypeKind::Primitive)
    }
}

#[derive(Debug, Clone)]
pub enum ModelTypeKind {
    Primitive,
    Composite {
        fields: Vec<ModelField>,
        table_id: Id<PhysicalTable>,
        pk_query: Id<Query>,
        collection_query: Id<Query>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelTypeModifier {
    Optional,
    NonNull,
    List,
}

#[derive(Debug, Clone)]
pub struct ModelField {
    pub name: String,
    pub typ: ModelFieldType,
    pub relation: ModelRelation,
}

#[derive(Debug, Clone)]
pub enum ModelFieldType {
    Optional(Box<ModelFieldType>),
    Reference {
        type_id: Id<ModelType>,
        type_name: String,
    },
    List(Box<ModelFieldType>),
}

impl ModelFieldType {
    pub fn type_id(&self) -> &Id<ModelType> {
        match self {
            ModelFieldType::Optional(underlying) | ModelFieldType::List(underlying) => {
                underlying.type_id()
            }
            ModelFieldType::Reference { type_id, .. } => type_id,
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            ModelFieldType::Optional(underlying) | ModelFieldType::List(underlying) => {
                underlying.type_name()
            }
            ModelFieldType::Reference { type_name, .. } => type_name,
        }
    }

    pub fn optional(&self) -> Self {
        match self {
            ModelFieldType::Optional(_) => self.clone(),
            _ => ModelFieldType::Optional(Box::new(self.clone())),
        }
    }
}
