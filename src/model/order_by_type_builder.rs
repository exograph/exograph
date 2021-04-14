use id_arena::Id;

use super::column_id::ColumnId;

use super::{order::*, relation::ModelRelation, system_context::SystemContextBuilding, types::*};

pub fn build(building: &mut SystemContextBuilding) {
    let primitive_type = OrderByParameterType {
        name: "Ordering".to_string(),
        kind: OrderByParameterTypeKind::Primitive,
    };

    building
        .order_by_types
        .add(&primitive_type.name.clone(), primitive_type);

    for model_type in building.types.iter() {
        let shallow_type = create_shallow_type(model_type.1);
        let param_type_name = shallow_type.name.clone();
        building.order_by_types.add(&param_type_name, shallow_type);
    }

    for model_type in building.types.iter() {
        let existing_model_type = building.types.get_by_key(&model_type.1.name);

        match existing_model_type {
            Some(existing_model_type) => {
                let param_type_name = get_parameter_type_name(existing_model_type);
                let existing_param_id = building.order_by_types.get_id(&param_type_name);

                let new_kind = expand_type(&model_type.1, &building);
                building.order_by_types.values[existing_param_id.unwrap()].kind = new_kind;
            }
            None => panic!(""),
        }
    }
}

pub fn get_parameter_type_name(model_type: &ModelType) -> String {
    match &model_type.kind {
        ModelTypeKind::Primitive => "Ordering".to_string(),
        ModelTypeKind::Composite { .. } => format!("{}Ordering", &model_type.name),
    }
}

fn create_shallow_type(model_type: &ModelType) -> OrderByParameterType {
    OrderByParameterType {
        name: get_parameter_type_name(model_type),
        kind: OrderByParameterTypeKind::Composite { parameters: vec![] },
    }
}

fn expand_type(
    model_type: &ModelType,
    building: &SystemContextBuilding,
) -> OrderByParameterTypeKind {
    match &model_type.kind {
        ModelTypeKind::Primitive => OrderByParameterTypeKind::Primitive,
        ModelTypeKind::Composite { fields, .. } => {
            let parameters = fields
                .iter()
                .map(|field| new_field_param(field, building))
                .collect();

            OrderByParameterTypeKind::Composite { parameters }
        }
    }
}

fn new_param(
    name: &str,
    model_type: &ModelType,
    column_id: Option<ColumnId>,
    building: &SystemContextBuilding,
) -> OrderByParameter {
    let (param_type_name, param_type_id) = order_by_param_type(model_type, building);

    OrderByParameter {
        name: name.to_string(),
        type_name: param_type_name,
        type_id: param_type_id,
        // Specifying ModelTypeModifier::List allows queries such as:
        // order_by: [{name: ASC}, {id: DESC}]
        // Using a List is the only way to maintain ordering within a parameter value
        // (the order within an object is not guaranteed to be maintained (and the graphql-parser uses BTreeMap that doesn't maintain so))
        //
        // But this also allows nonsensical queries such as
        // order_by: [{name: ASC, id: DESC}].
        // Here the user intention is the same as the query above, but we cannot honor that intention
        // This seems like an inherent limit of GraphQL types system (perhaps, input union type proposal will help fix this)
        // TODO: When executing, check for the unsupported version (more than one attributes in an array element) and return an error
        type_modifier: ModelTypeModifier::List,
        column_id,
    }
}

pub fn new_field_param(
    model_field: &ModelField,
    building: &SystemContextBuilding,
) -> OrderByParameter {
    let field_model_type = building.types.get_by_id(model_field.type_id).unwrap();

    let column_id = match &model_field.relation {
        ModelRelation::Pk { column_id, .. } | ModelRelation::Scalar { column_id, .. } => {
            Some(column_id.clone())
        }
        _ => None,
    };

    new_param(&model_field.name, field_model_type, column_id, building)
}

pub fn new_root_param(
    model_type: &ModelType,
    building: &SystemContextBuilding,
) -> OrderByParameter {
    new_param("orderBy", model_type, None, building)
}

fn order_by_param_type(
    model_type: &ModelType,
    building: &SystemContextBuilding,
) -> (String, Id<OrderByParameterType>) {
    let param_type_name = get_parameter_type_name(model_type);
    let param_type_id = building.order_by_types.get_id(&param_type_name).unwrap();

    (param_type_name, param_type_id)
}
