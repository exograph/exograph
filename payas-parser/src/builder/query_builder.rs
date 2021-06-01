use id_arena::Id;
use payas_model::model::{
    operation::{OperationReturnType, Query},
    predicate::PredicateParameter,
    GqlType, GqlTypeKind, GqlTypeModifier,
};

use super::{order_by_type_builder, predicate_builder, system_builder::SystemContextBuilding, typechecking::{CompositeType, Type}};

pub fn build_shallow(models: &[Type], building: &mut SystemContextBuilding) {
    for model in models {
        if let Type::Composite(c) = &model {
            let model_type_id = building.types.get_id(c.name.as_str()).unwrap();
            let shallow_query = shallow_pk_query(model_type_id, c);
            let collection_query = shallow_collection_query(model_type_id, c);

            building
                .queries
                .add(&shallow_query.name.to_owned(), shallow_query);
            building
                .queries
                .add(&collection_query.name.to_owned(), collection_query);
        }
    }
}

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (_, model_type) in building.types.iter() {
        if let GqlTypeKind::Composite { .. } = &model_type.kind {
            {
                let operation_name = pk_query_name(&model_type.name);
                let query = expanded_pk_query(model_type, building);
                let existing_id = building.queries.get_id(&operation_name).unwrap();
                building.queries[existing_id] = query;
            }
            {
                let operation_name = collection_query_name(&model_type.name);
                let query = expanded_collection_query(model_type, building);
                let existing_id = building.queries.get_id(&operation_name).unwrap();
                building.queries[existing_id] = query;
            }
        }
    }
}

fn shallow_pk_query(model_type_id: Id<GqlType>, typ: &CompositeType) -> Query {
    let operation_name = pk_query_name(typ.name.as_str());
    Query {
        name: operation_name,
        predicate_param: None,
        order_by_param: None,
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: typ.name.clone(),
            type_modifier: GqlTypeModifier::NonNull,
        },
    }
}

fn expanded_pk_query(model_type: &GqlType, building: &SystemContextBuilding) -> Query {
    let operation_name = pk_query_name(&model_type.name);
    let existing_query = building.queries.get_by_key(&operation_name).unwrap();

    let pk_param = pk_predicate_param(model_type, building);

    Query {
        name: operation_name,
        predicate_param: Some(pk_param),
        order_by_param: None,
        return_type: existing_query.return_type.clone(),
    }
}

pub fn pk_predicate_param(
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> PredicateParameter {
    let pk_field = model_type.pk_field().unwrap();

    PredicateParameter {
        name: pk_field.name.to_string(),
        type_name: pk_field.typ.type_name().to_string(),
        type_id: building
            .predicate_types
            .get_id(&pk_field.typ.type_name())
            .unwrap(),
        type_modifier: GqlTypeModifier::NonNull,
        column_id: pk_field.relation.self_column(),
    }
}

fn shallow_collection_query(model_type_id: Id<GqlType>, model: &CompositeType) -> Query {
    let operation_name = collection_query_name(model.name.as_str());
    Query {
        name: operation_name,
        predicate_param: None,
        order_by_param: None,
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: model.name.clone(),
            type_modifier: GqlTypeModifier::List,
        },
    }
}

fn expanded_collection_query(model_type: &GqlType, building: &SystemContextBuilding) -> Query {
    let operation_name = collection_query_name(&model_type.name);
    let existing_query = building.queries.get_by_key(&operation_name).unwrap();

    let predicate_param = collection_predicate_param(model_type, building);
    let order_by_param = order_by_type_builder::new_root_param(&model_type.name, false, building);

    Query {
        name: operation_name.clone(),
        predicate_param: Some(predicate_param),
        order_by_param: Some(order_by_param),
        return_type: existing_query.return_type.clone(),
    }
}

pub fn collection_predicate_param(
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> PredicateParameter {
    let param_type_name = predicate_builder::get_parameter_type_name(&model_type.name);
    PredicateParameter {
        name: "where".to_string(),
        type_name: param_type_name.clone(),
        type_id: building.predicate_types.get_id(&param_type_name).unwrap(),
        type_modifier: GqlTypeModifier::Optional,
        column_id: None,
    }
}

pub fn pk_query_name(name: &str) -> String {
    // Concert -> concert, SavingsAccount -> savingsAccount i.e. lowercase the first letter
    let mut ret = name.to_owned();
    if let Some(r) = ret.get_mut(0..1) {
        r.make_ascii_lowercase();
    }
    ret
}

// TODO: Bring in a proper pluralize implementation
pub fn collection_query_name(input: &str) -> String {
    format!("{}s", pk_query_name(input))
}
