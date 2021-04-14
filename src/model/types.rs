use super::relation::ModelRelation;
use crate::model::operation::*;

use crate::sql::table::PhysicalTable;

use super::{
    order_by_type_builder, predicate::PredicateParameter, predicate_builder,
    system_context::SystemContextBuilding,
};
use id_arena::Id;
#[derive(Debug, Clone)]
pub struct ModelType {
    pub name: String,
    pub kind: ModelTypeKind,
}

impl ModelType {
    pub fn model_field(&self, name: &str) -> Option<&ModelField> {
        match &self.kind {
            ModelTypeKind::Primitive => None,
            ModelTypeKind::Composite { fields, .. } => {
                fields.iter().find(|model_field| model_field.name == name)
            }
        }
    }

    pub fn pk_field(&self) -> Option<&ModelField> {
        match &self.kind {
            ModelTypeKind::Primitive => None,
            ModelTypeKind::Composite { fields, .. } => fields.iter().find_map(|field| {
                if let ModelRelation::Pk { .. } = &field.relation {
                    Some(field)
                } else {
                    None
                }
            }),
        }
    }

    pub fn queries(&self, building: &SystemContextBuilding) -> Vec<Query> {
        match &self.kind {
            ModelTypeKind::Primitive => vec![],
            ModelTypeKind::Composite { .. } => {
                vec![self.by_pk_query(building), self.collection_query(building)]
            }
        }
    }

    fn by_pk_query(&self, building: &SystemContextBuilding) -> Query {
        let operation_name = self.normalized_name().to_owned();
        let return_type_id = building.types.get_id(&self.name).unwrap();
        let return_type: OperationReturnType = OperationReturnType {
            type_name: self.name.to_owned(),
            type_id: return_type_id,
            type_modifier: ModelTypeModifier::NonNull,
        };

        let pk_field = self.pk_field().unwrap();

        let id_param = PredicateParameter {
            name: pk_field.name.to_string(),
            type_name: pk_field.type_name.to_string(),
            type_id: building
                .predicate_types
                .get_id(&pk_field.type_name)
                .unwrap()
                .clone(),
            type_modifier: ModelTypeModifier::NonNull,
            column_id: pk_field.relation.self_column(),
        };

        Query {
            name: operation_name,
            predicate_parameter: Some(id_param),
            order_by_param: None,
            return_type: return_type,
        }
    }

    fn collection_query(&self, building: &SystemContextBuilding) -> Query {
        let operation_name = to_plural(self.normalized_name());

        let return_type_id = building.types.get_id(&self.name).unwrap();
        let return_type: OperationReturnType = OperationReturnType {
            type_id: return_type_id,
            type_name: self.name.to_owned(),
            type_modifier: ModelTypeModifier::List,
        };

        let param_type_name = predicate_builder::get_parameter_type_name(&self.name);
        let predicate_param = PredicateParameter {
            name: "where".to_string(),
            type_name: param_type_name.clone(),
            type_id: building
                .predicate_types
                .get_id(&param_type_name)
                .unwrap()
                .clone(),
            type_modifier: ModelTypeModifier::Optional,
            column_id: None,
        };

        let order_by_param = order_by_type_builder::new_root_param(&self, building);

        Query {
            name: operation_name.clone(),
            predicate_parameter: Some(predicate_param),
            order_by_param: Some(order_by_param),
            return_type: return_type,
        }
    }

    fn normalized_name(&self) -> String {
        // Concert -> concert, SavingsAccount -> savingsAccount i.e. lowercase the first letter
        let mut ret = self.name.to_owned();
        if let Some(r) = ret.get_mut(0..1) {
            r.make_ascii_lowercase();
        }
        ret
    }
}

#[derive(Debug, Clone)]
pub enum ModelTypeKind {
    Primitive,
    Composite {
        fields: Vec<ModelField>,
        table_id: Id<PhysicalTable>,
    },
}

impl ModelTypeKind {
    fn empty_composite(table_id: Id<PhysicalTable>) -> Self {
        ModelTypeKind::Composite {
            fields: vec![],
            table_id,
        }
    }
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
    pub type_id: Id<ModelType>,
    pub type_name: String,
    pub type_modifier: ModelTypeModifier,
    pub relation: ModelRelation,
}

// TODO: Bring in a proper pluralize implementation
fn to_plural(input: String) -> String {
    format!("{}s", input)
}
