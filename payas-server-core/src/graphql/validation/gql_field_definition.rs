use async_graphql_parser::types::{FieldDefinition, InputValueDefinition};
use payas_model::model::{
    operation::{
        DatabaseMutationKind, DatabaseQueryParameter, Mutation, MutationKind, OperationReturnType,
        Query, QueryKind,
    },
    relation::GqlRelation,
    system::ModelSystem,
    GqlField, GqlFieldType, GqlTypeKind, GqlTypeModifier,
};

use super::definition::{GqlFieldDefinition, GqlFieldTypeDefinition, GqlFieldTypeDefinitionNode};

impl GqlFieldTypeDefinition for OperationReturnType {
    fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
        &self.type_name
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
        GqlFieldTypeDefinitionNode::NonLeaf(self.typ(model), &self.type_modifier)
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

impl GqlFieldDefinition for Query {
    fn name(&self) -> &str {
        &self.name
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.return_type
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        match &self.kind {
            QueryKind::Database(db_query_param) => {
                compute_db_arg_definition(db_query_param.as_ref())
            }
            QueryKind::Service { argument_param, .. } => argument_param
                .iter()
                .map(|arg| arg as &dyn GqlFieldDefinition)
                .collect(),
        }
    }
}

impl GqlFieldDefinition for Mutation {
    fn name(&self) -> &str {
        &self.name
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.return_type
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        match &self.kind {
            MutationKind::Database { kind } => match kind {
                DatabaseMutationKind::Create(param) => vec![param],
                DatabaseMutationKind::Delete(param) => vec![param],
                DatabaseMutationKind::Update {
                    data_param,
                    predicate_param,
                } => vec![data_param, predicate_param],
            },
            MutationKind::Service { argument_param, .. } => argument_param
                .iter()
                .map(|p| p as &dyn GqlFieldDefinition)
                .collect(),
        }
    }
}

impl GqlFieldTypeDefinition for GqlFieldType {
    fn name<'a>(&'a self, model: &'a ModelSystem) -> &'a str {
        match self {
            GqlFieldType::Optional(ty) | GqlFieldType::List(ty) => {
                GqlFieldTypeDefinition::name(ty.as_ref(), model)
            }
            GqlFieldType::Reference { type_name, .. } => type_name,
        }
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
        match self {
            GqlFieldType::Optional(ty) => {
                GqlFieldTypeDefinitionNode::NonLeaf(ty.as_ref(), &GqlTypeModifier::Optional)
            }
            GqlFieldType::List(ty) => {
                GqlFieldTypeDefinitionNode::NonLeaf(ty.as_ref(), &GqlTypeModifier::List)
            }
            GqlFieldType::Reference { type_id, .. } => {
                GqlFieldTypeDefinitionNode::Leaf(&model.types[*type_id])
            }
        }
    }

    fn modifier(&self) -> &GqlTypeModifier {
        match self {
            GqlFieldType::Optional(_) => &GqlTypeModifier::Optional,
            GqlFieldType::List(_) => &GqlTypeModifier::List,
            _ => &GqlTypeModifier::NonNull,
        }
    }
}

impl GqlFieldDefinition for GqlField {
    fn name(&self) -> &str {
        &self.name
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.typ
    }

    fn arguments<'a>(&'a self, model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        match self.relation {
            GqlRelation::Pk { .. }
            | GqlRelation::Scalar { .. }
            | GqlRelation::ManyToOne { .. }
            | GqlRelation::NonPersistent => {
                vec![]
            }
            GqlRelation::OneToMany { other_type_id, .. } => {
                let other_type = &model.types[other_type_id];
                match &other_type.kind {
                    GqlTypeKind::Primitive => panic!(),
                    GqlTypeKind::Composite(kind) => {
                        let collection_query = kind.get_collection_query();
                        let collection_query = &model.queries[collection_query];

                        match &collection_query.kind {
                            QueryKind::Database(db_query_params) => {
                                compute_db_arg_definition(db_query_params)
                            }
                            QueryKind::Service { .. } => panic!(),
                        }
                    }
                }
            }
        }
    }
}

impl GqlFieldDefinition for FieldDefinition {
    fn name(&self) -> &str {
        self.name.node.as_str()
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.ty.node
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        self.arguments
            .iter()
            .map(|arg| &arg.node as &dyn GqlFieldDefinition)
            .collect()
    }
}

impl GqlFieldDefinition for InputValueDefinition {
    fn name(&self) -> &str {
        self.name.node.as_str()
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.ty.node
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![]
    }
}

pub(super) fn compute_db_arg_definition(
    db_query_params: &DatabaseQueryParameter,
) -> Vec<&dyn GqlFieldDefinition> {
    let DatabaseQueryParameter {
        predicate_param,
        order_by_param,
        limit_param,
        offset_param,
    } = db_query_params;

    vec![
        predicate_param
            .as_ref()
            .map(|p| p as &dyn GqlFieldDefinition),
        order_by_param
            .as_ref()
            .map(|p| p as &dyn GqlFieldDefinition),
        limit_param.as_ref().map(|p| p as &dyn GqlFieldDefinition),
        offset_param.as_ref().map(|p| p as &dyn GqlFieldDefinition),
    ]
    .into_iter()
    .flatten()
    .collect()
}
