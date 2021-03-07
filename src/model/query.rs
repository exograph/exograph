use crate::model::operation::*;
use crate::model::types::*;

use super::{
    order::*,
    predicate::PredicateParameter,
    system::{ModelSystem, ModelSystemParameterTypes},
};

pub trait QueryProvider {
    fn queries(
        &self,
        system: &ModelSystem,
        // Really a part of the system, but have to separate out to allow mutating it while still acceesing system
        system_param_types: &mut ModelSystemParameterTypes,
    ) -> Vec<Query>;
}

impl QueryProvider for ModelType {
    fn queries(
        &self,
        system: &ModelSystem,
        system_param_types: &mut ModelSystemParameterTypes,
    ) -> Vec<Query> {
        match &self.kind {
            ModelTypeKind::Primitive => vec![],
            ModelTypeKind::Composite { .. } => {
                vec![
                    by_pk_query(self, system, system_param_types),
                    collection_query(self, system, system_param_types),
                ]
            }
        }
    }
}

fn by_pk_query(
    tpe: &ModelType,
    _system: &ModelSystem,
    _system_param_types: &mut ModelSystemParameterTypes,
) -> Query {
    let operation_name = normalized_name(tpe).to_owned();

    let return_type: OperationReturnType = OperationReturnType {
        type_name: tpe.name.clone(),
        type_modifier: ModelTypeModifier::NonNull,
    };

    let id_param = PredicateParameter::new_pk(&tpe.name, _system, _system_param_types);

    Query {
        name: operation_name,
        predicate_parameter: Some(id_param),
        order_by_param: None,
        return_type: return_type,
    }
}

fn collection_query(
    tpe: &ModelType,
    system: &ModelSystem,
    param_types: &mut ModelSystemParameterTypes,
) -> Query {
    let operation_name = to_plural(normalized_name(tpe));

    let return_type: OperationReturnType = OperationReturnType {
        type_name: tpe.name.clone(),
        type_modifier: ModelTypeModifier::List,
    };

    Query {
        name: operation_name.clone(),
        predicate_parameter: Some(PredicateParameter::new_collection(
            &tpe.name,
            "where",
            system,
            param_types,
        )),
        order_by_param: Some(OrderByParameter::new(
            &tpe.name,
            "orderBy".to_string(),
            system,
            param_types,
        )),
        return_type: return_type,
    }
}

fn normalized_name(tpe: &ModelType) -> String {
    // Concert -> concert i.e. lowercase the first letter
    let mut ret = tpe.name.to_owned();
    if let Some(r) = ret.get_mut(0..1) {
        r.make_ascii_lowercase();
    }
    ret
}

// TODO: Bring in a proper pluralize implementation
fn to_plural(input: String) -> String {
    format!("{}s", input)
}
