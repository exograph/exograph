use id_arena::Id;

use super::{
    ast::ast_types::{AstType, AstTypeKind},
    operation::{OperationReturnType, Query},
    order_by_type_builder,
    predicate::PredicateParameter,
    predicate_builder,
    system_context::SystemContextBuilding,
    types::{ModelType, ModelTypeKind, ModelTypeModifier},
};

pub fn build_shallow(ast_types: &[AstType], building: &mut SystemContextBuilding) {
    for ast_type in ast_types.iter() {
        if let AstTypeKind::Composite { .. } = &ast_type.kind {
            let model_type_id = building.types.get_id(&ast_type.name).unwrap();
            let shallow_query = shallow_pk_query(model_type_id, ast_type);
            let collection_query = shallow_collection_query(model_type_id, ast_type);

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
        if let ModelTypeKind::Composite { .. } = &model_type.kind {
            {
                let operation_name = pk_query_name(&model_type.name).to_owned();
                let query = expanded_pk_query(model_type, building);
                let existing_id = building.queries.get_id(&operation_name).unwrap();
                building.queries[existing_id] = query;
            }
            {
                let operation_name = collection_query_name(&model_type.name).to_owned();
                let query = expanded_collection_query(model_type, building);
                let existing_id = building.queries.get_id(&operation_name).unwrap();
                building.queries[existing_id] = query;
            }
        }
    }
}

fn shallow_pk_query(model_type_id: Id<ModelType>, ast_type: &AstType) -> Query {
    let operation_name = pk_query_name(&ast_type.name).to_owned();
    Query {
        name: operation_name,
        predicate_param: None,
        order_by_param: None,
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: ast_type.name.clone(),
            type_modifier: ModelTypeModifier::NonNull,
        },
    }
}

fn expanded_pk_query(model_type: &ModelType, building: &SystemContextBuilding) -> Query {
    let operation_name = pk_query_name(&model_type.name).to_owned();
    let existing_query = building.queries.get_by_key(&operation_name).unwrap();

    let pk_field = model_type.pk_field().unwrap();

    let pk_param = PredicateParameter {
        name: pk_field.name.to_string(),
        type_name: pk_field.type_name.to_string(),
        type_id: building
            .predicate_types
            .get_id(&pk_field.type_name)
            .unwrap(),
        type_modifier: ModelTypeModifier::NonNull,
        column_id: pk_field.relation.self_column(),
    };

    Query {
        name: operation_name,
        predicate_param: Some(pk_param),
        order_by_param: None,
        return_type: existing_query.return_type.clone(),
    }
}

fn shallow_collection_query(model_type_id: Id<ModelType>, ast_type: &AstType) -> Query {
    let operation_name = collection_query_name(&ast_type.name).to_owned();
    Query {
        name: operation_name,
        predicate_param: None,
        order_by_param: None,
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: ast_type.name.clone(),
            type_modifier: ModelTypeModifier::List,
        },
    }
}

fn expanded_collection_query(model_type: &ModelType, building: &SystemContextBuilding) -> Query {
    let operation_name = collection_query_name(&model_type.name).to_owned();
    let existing_query = building.queries.get_by_key(&operation_name).unwrap();

    let param_type_name = predicate_builder::get_parameter_type_name(&model_type.name);
    let predicate_param = PredicateParameter {
        name: "where".to_string(),
        type_name: param_type_name.clone(),
        type_id: building.predicate_types.get_id(&param_type_name).unwrap(),
        type_modifier: ModelTypeModifier::Optional,
        column_id: None,
    };

    let order_by_param = order_by_type_builder::new_root_param(&model_type.name, false, building);

    Query {
        name: operation_name.clone(),
        predicate_param: Some(predicate_param),
        order_by_param: Some(order_by_param),
        return_type: existing_query.return_type.clone(),
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
