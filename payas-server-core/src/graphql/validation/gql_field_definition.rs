use payas_model::model::{
    operation::{
        DatabaseMutationKind, DatabaseQueryParameter, Mutation, MutationKind, OperationReturnType,
        Query, QueryKind,
    },
    relation::GqlRelation,
    system::ModelSystem,
    GqlField, GqlFieldType, GqlTypeKind, GqlTypeModifier,
};

use super::definition::{GqlFieldDefinition, GqlFieldTypeDefinition, GqlTypeDefinition};

// pub enum GqlFieldTypeDefinitionY<'a> {
//     Base(&'a dyn GqlTypeDefinition),
//     Optional(&'a GqlFieldTypeDefinitionY<'a>),
//     List(&'a GqlFieldTypeDefinitionY<'a>),
// }

// pub trait GqlFieldTypeDefinitionYProvider {
//     fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionY<'a>;
// }

// impl GqlFieldTypeDefinitionYProvider for OperationReturnType {
//     fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionY<'a> {
//         match self.type_modifier {
//             GqlTypeModifier::Optional => {
//                 GqlFieldTypeDefinitionY::Optional(&self.typ(model).inner(model))
//             }
//             GqlTypeModifier::List => GqlFieldTypeDefinitionY::List(&self.typ(model).inner(model)),
//             _ => GqlFieldTypeDefinitionY::Base(self.typ(model)),
//         }
//     }
// }

// impl GqlFieldTypeDefinitionYProvider for GqlType {
//     fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionY<'a> {
//         GqlFieldTypeDefinitionY::Base(self)
//     }
// }

// pub enum GqlFieldTypeDefinitionX<'a> {
//     Base(&'a dyn GqlTypeDefinition),
//     Optional(Box<GqlFieldTypeDefinitionX<'a>>),
//     List(Box<GqlFieldTypeDefinitionX<'a>>),
// }

// impl<'a> From<&'a OperationReturnType> for GqlFieldTypeDefinitionX<'a> {
//     fn from(ty: &'a OperationReturnType) -> Self {
//         match ty.type_modifier {
//             GqlTypeModifier::NonNull => GqlFieldTypeDefinitionX::Base(ty),
//             GqlTypeModifier::Optional => {
//                 GqlFieldTypeDefinitionX::Optional(Box::new(GqlFieldTypeDefinitionX::from(ty)))
//             }
//             GqlTypeModifier::List => {
//                 GqlFieldTypeDefinitionX::List(Box::new(GqlFieldTypeDefinitionX::from(ty)))
//             }
//         }
//     }
// }

impl GqlFieldTypeDefinition for OperationReturnType {
    fn name(&self) -> &str {
        &self.type_name
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        Some(self.typ(model))
    }

    fn leaf<'a>(&'a self, model: &'a ModelSystem) -> &'a dyn GqlTypeDefinition {
        self.typ(model)
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

// impl<'a> From<&'a OperationReturnType> for GqlFieldTypeDefinition<'a> {
//     fn from(ty: &'a OperationReturnType) -> Self {
//         match ty.type_modifier {
//             GqlTypeModifier::NonNull => GqlFieldTypeDefinition::Primitive(ty.),
//             GqlTypeModifier::Optional => {
//                 GqlFieldTypeDefinition::Optional(GqlFieldTypeDefinition::from(t))
//             }
//             GqlTypeModifier::List => GqlFieldTypeDefinition::List(GqlFieldTypeDefinition::from(t)),
//         }
//     }
// }

// impl<'a> From<&'a GqlFieldType> for GqlFieldTypeDefinition<'a> {
//     fn from(ty: &'a GqlFieldType) -> Self {
//         match ty {
//             GqlFieldType::Optional(ty) => GqlFieldTypeDefinition::Optional(ty.as_ref().into()),
//             GqlFieldType::List(ty) => GqlFieldTypeDefinition::List(ty.as_ref().into()),
//             _ => GqlFieldTypeDefinition::Primitive(ty),
//         }
//     }
// }

impl GqlFieldDefinition for Query {
    fn name(&self) -> &str {
        &self.name
    }

    fn ty<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.return_type
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        match &self.kind {
            QueryKind::Database(db_query_param) => {
                compute_db_arg_definition(db_query_param.as_ref())
            }
            QueryKind::Service { .. } => {
                todo!()
            }
        }
    }
}

impl GqlFieldDefinition for Mutation {
    fn name(&self) -> &str {
        &self.name
    }

    fn ty<'a>(&'a self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
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
            MutationKind::Service { .. } => {
                todo!()
            }
        }
    }
}

impl GqlFieldTypeDefinition for GqlFieldType {
    fn name(&self) -> &str {
        match self {
            GqlFieldType::Optional(ty) | GqlFieldType::List(ty) => {
                GqlFieldTypeDefinition::name(ty.as_ref())
            }
            GqlFieldType::Reference { type_name, .. } => type_name,
        }
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        match self {
            GqlFieldType::Optional(ty) => Some(ty.as_ref()),
            GqlFieldType::List(ty) => Some(ty.as_ref()),
            _ => None,
        }
    }

    fn leaf<'a>(&'a self, model: &'a ModelSystem) -> &'a dyn GqlTypeDefinition {
        match self {
            GqlFieldType::Optional(ty) => ty.leaf(model),
            GqlFieldType::List(ty) => ty.leaf(model),
            GqlFieldType::Reference { type_id, type_name } => &model.types[*type_id],
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

    fn ty<'a>(&'a self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
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
