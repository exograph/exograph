use async_graphql_parser::types::{
    BaseType, FieldDefinition, ObjectType, Type, TypeDefinition, TypeKind,
};
use async_graphql_value::Name;
use serde::{Deserialize, Serialize};

use crate::model::ModelPostgresSystem;
use crate::operation::AggregateQueryParameter;
use crate::relation::PostgresRelation;
use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;
use core_plugin_interface::core_model::type_normalization::{
    default_positioned, default_positioned_name, FieldDefinitionProvider, InputValueProvider,
    TypeDefinitionProvider,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct AggregateType {
    pub name: String, // Such as IntAgg, ConcertAgg.
    pub fields: Vec<AggregateField>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AggregateField {
    pub name: String, // Such as max, sum, etc for scalar types; field names (id, name, etc.) for composite types
    pub typ: AggregateFieldType,
    pub relation: Option<PostgresRelation>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AggregateFieldType {
    Scalar {
        type_name: String,              // "Int", "String", etc.
        kind: ScalarAggregateFieldKind, // Min, Max, Sum, etc.
    },
    Composite {
        type_name: String,
        type_id: SerializableSlabIndex<AggregateType>,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum ScalarAggregateFieldKind {
    Avg,
    Count,
    Max,
    Min,
    Sum,
}

impl ScalarAggregateFieldKind {
    pub fn name(&self) -> &str {
        match self {
            ScalarAggregateFieldKind::Avg => "avg",
            ScalarAggregateFieldKind::Count => "count",
            ScalarAggregateFieldKind::Max => "max",
            ScalarAggregateFieldKind::Min => "min",
            ScalarAggregateFieldKind::Sum => "sum",
        }
    }
}

impl TypeDefinitionProvider<ModelPostgresSystem> for AggregateType {
    fn type_definition(&self, system: &ModelPostgresSystem) -> TypeDefinition {
        let kind = {
            let fields: Vec<_> = self
                .fields
                .iter()
                .map(|field| default_positioned(field.field_definition(system)))
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

impl FieldDefinitionProvider<ModelPostgresSystem> for AggregateField {
    fn field_definition(&self, system: &ModelPostgresSystem) -> FieldDefinition {
        let arguments = match &self.relation {
            Some(relation) => match relation {
                PostgresRelation::Pk { .. }
                | PostgresRelation::Scalar { .. }
                | PostgresRelation::ManyToOne { .. } => {
                    vec![]
                }
                PostgresRelation::OneToMany { other_type_id, .. } => {
                    let other_type = &system.entity_types[*other_type_id];
                    let aggregate_query = &system.aggregate_queries[other_type.aggregate_query];

                    let AggregateQueryParameter { predicate_param } = &aggregate_query.parameter;

                    vec![default_positioned(predicate_param.input_value())]
                }
            },
            None => vec![],
        };

        FieldDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            arguments,
            ty: default_positioned(compute_type(&self.typ)),
            directives: vec![],
        }
    }
}

fn compute_type(typ: &AggregateFieldType) -> Type {
    let base = match typ {
        AggregateFieldType::Scalar { type_name, .. } => BaseType::Named(Name::new(type_name)),
        AggregateFieldType::Composite { type_name, .. } => BaseType::Named(Name::new(type_name)),
    };

    Type {
        base,
        nullable: true,
    }
}
