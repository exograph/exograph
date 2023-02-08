use crate::{
    column_path::ColumnIdPathLink,
    model::ModelPostgresSystem,
    types::{EntityType, TypeIndex},
};
use async_graphql_parser::types::{InputObjectType, TypeDefinition, TypeKind};
use serde::{Deserialize, Serialize};

use super::types::PostgresTypeModifier;
use core_plugin_interface::core_model::{
    mapped_arena::SerializableSlabIndex,
    type_normalization::{
        default_positioned, default_positioned_name, InputValueProvider, Parameter, ParameterType,
        TypeDefinitionProvider, TypeModifier,
    },
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PredicateParameter {
    /// The name of the parameter. For example, "where", "and", "id", "venue", etc.
    pub name: String,
    /// The type name of the parameter.
    /// For example, "ConcertFilter", "IntFilter". We need to keep this only for introspection, which doesn't have access to the ModelSystem.
    /// We might find a way to avoid this, since given the model system and type_id of the parameter, we can get the type name.
    pub type_name: String,

    pub typ: PredicateParameterTypeWithModifier,

    /// How does this parameter relates with the parent parameter?
    /// For example for parameter used as {where: {venue1: {id: {eq: 1}}}}, we will have following column links:
    /// eq: None
    /// id: Some((<the venues.id column>, None))
    /// venue1: Some((<the concerts.venue1_id column>, <the venues.id column>))
    /// where: None
    pub column_path_link: Option<ColumnIdPathLink>,

    /// The type this parameter is filtering on. For example, for ConcertFilter, this will be (the index of) the Concert.
    pub underlying_type_id: TypeIndex<EntityType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PredicateParameterTypeWithModifier {
    /// The type modifier of the parameter. For parameters such as "and", this will be a list.
    pub type_modifier: PostgresTypeModifier,
    /// Type id of the parameter type. For example: IntFilter, StringFilter, etc.
    pub type_id: SerializableSlabIndex<PredicateParameterType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PredicateParameterType {
    pub name: String,
    pub kind: PredicateParameterTypeKind,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PredicateParameterTypeKind {
    ImplicitEqual,                     // {id: 3}
    Operator(Vec<PredicateParameter>), // {lt: ..,gt: ..} such as IntFilter
    Composite {
        field_params: Vec<PredicateParameter>, // {where: {id: .., name: ..}} such as AccountFilter
        logical_op_params: Vec<PredicateParameter>, // logical operator predicates like `and: [{name: ..}, {id: ..}]`
    },
}

impl ParameterType for PredicateParameterType {
    fn name(&self) -> &String {
        &self.name
    }
}

impl Parameter for PredicateParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_modifier(&self) -> TypeModifier {
        (&self.typ.type_modifier).into()
    }
}

impl TypeDefinitionProvider<ModelPostgresSystem> for PredicateParameterType {
    fn type_definition(&self, _system: &ModelPostgresSystem) -> TypeDefinition {
        match &self.kind {
            PredicateParameterTypeKind::Operator(parameters) => {
                let fields = parameters
                    .iter()
                    .map(|parameter| default_positioned(parameter.input_value()))
                    .collect();

                TypeDefinition {
                    extend: false,
                    description: None,
                    name: default_positioned_name(self.name()),
                    directives: vec![],
                    kind: TypeKind::InputObject(InputObjectType { fields }),
                }
            }
            PredicateParameterTypeKind::Composite {
                field_params,
                logical_op_params,
            } => {
                let parameters = [field_params, &logical_op_params[..]].concat();

                let fields = parameters
                    .iter()
                    .map(|parameter| default_positioned(parameter.input_value()))
                    .collect();

                TypeDefinition {
                    extend: false,
                    description: None,
                    name: default_positioned_name(self.name()),
                    directives: vec![],
                    kind: TypeKind::InputObject(InputObjectType { fields }),
                }
            }
            PredicateParameterTypeKind::ImplicitEqual => TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(self.name()),
                directives: vec![],
                kind: TypeKind::Scalar,
            },
        }
    }
}
