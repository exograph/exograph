use super::{column_id::ColumnId, relation::GqlRelation};
use crate::model::operation::*;

use crate::sql::PhysicalTable;

use id_arena::Id;

#[derive(Debug, Clone)]
pub struct ContextType {
    pub name: String,
    pub fields: Vec<ContextField>,
}

#[derive(Debug, Clone)]
pub struct ContextField {
    pub name: String,
    pub typ: GqlFieldType,
    pub source: ContextSource,
}

#[derive(Debug, Clone)]
pub enum ContextSource {
    Jwt { claim: String },
}

#[derive(Debug, Clone)]
pub struct GqlType {
    pub name: String,
    pub kind: GqlTypeKind,
    pub is_input: bool, // Is this to be used as an input field (such as an argument in a mutation)? Needed for introspection
}

impl GqlType {
    pub fn model_fields(&self) -> Vec<&GqlField> {
        match &self.kind {
            GqlTypeKind::Primitive => vec![],
            GqlTypeKind::Composite { fields, .. } => fields.iter().collect(),
        }
    }

    pub fn model_field(&self, name: &str) -> Option<&GqlField> {
        self.model_fields()
            .into_iter()
            .find(|model_field| model_field.name == name)
    }

    pub fn pk_field(&self) -> Option<&GqlField> {
        self.model_fields().iter().find_map(|field| {
            if let GqlRelation::Pk { .. } = &field.relation {
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
            GqlTypeKind::Primitive => None,
            GqlTypeKind::Composite { table_id, .. } => Some(*table_id),
        }
    }

    pub fn is_primitive(&self) -> bool {
        matches!(&self.kind, GqlTypeKind::Primitive)
    }
}

#[derive(Debug, Clone)]
pub enum GqlTypeKind {
    Primitive,
    Composite {
        fields: Vec<GqlField>,
        table_id: Id<PhysicalTable>,
        pk_query: Id<Query>,
        collection_query: Id<Query>,
        //access: Option<AccessExpression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum GqlTypeModifier {
    Optional,
    NonNull,
    List,
}

#[derive(Debug, Clone)]
pub struct GqlField {
    pub name: String,
    pub typ: GqlFieldType,
    pub relation: GqlRelation,
}

#[derive(Debug, Clone)]
pub enum GqlFieldType {
    Optional(Box<GqlFieldType>),
    Reference {
        type_id: Id<GqlType>,
        type_name: String,
    },
    List(Box<GqlFieldType>),
}

impl GqlFieldType {
    pub fn type_id(&self) -> &Id<GqlType> {
        match self {
            GqlFieldType::Optional(underlying) | GqlFieldType::List(underlying) => {
                underlying.type_id()
            }
            GqlFieldType::Reference { type_id, .. } => type_id,
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            GqlFieldType::Optional(underlying) | GqlFieldType::List(underlying) => {
                underlying.type_name()
            }
            GqlFieldType::Reference { type_name, .. } => type_name,
        }
    }

    pub fn optional(&self) -> Self {
        match self {
            GqlFieldType::Optional(_) => self.clone(),
            _ => GqlFieldType::Optional(Box::new(self.clone())),
        }
    }
}
