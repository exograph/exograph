use super::{predicate::*, system_context::SystemContextBuilding, types::*};

pub fn build(building: &mut SystemContextBuilding) {
    for model_type in building.types.iter() {
        match model_type.1.kind {
            ModelTypeKind::Primitive => {
                building.predicate_types.add(
                    &model_type.1.name,
                    PredicateParameterType {
                        name: model_type.1.name.clone(),
                        kind: PredicateParameterTypeKind::ImplicitEqual {},
                    },
                );
            }
            _ => (),
        }

        let shallow_type = create_shallow_type(model_type.1);
        let param_type_name = shallow_type.name.clone();
        building.predicate_types.add(&param_type_name, shallow_type);
    }

    for model_type in building.types.iter() {
        let existing_model_type = building.types.get_by_key(&model_type.1.name);

        match existing_model_type {
            Some(existing_model_type) => {
                let param_type_name = get_parameter_type_name(&existing_model_type.name);
                let existing_param_id = building.predicate_types.get_id(&param_type_name);

                let new_kind = expand_type(&model_type.1, building);
                building.predicate_types.values[existing_param_id.unwrap()].kind = new_kind;
            }
            None => panic!(""),
        }
    }
}

pub fn get_parameter_type_name(model_type_name: &str) -> String {
    format!("{}Filter", model_type_name)
}

fn create_shallow_type(model_type: &ModelType) -> PredicateParameterType {
    PredicateParameterType {
        name: get_parameter_type_name(&model_type.name),
        kind: PredicateParameterTypeKind::ImplicitEqual, // Will be set to the correct value in expand_type
    }
}

fn expand_type(
    model_type: &ModelType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    match &model_type.kind {
        ModelTypeKind::Primitive => create_operator_filter_type_kind(&model_type, building),
        ModelTypeKind::Composite { fields, .. } => {
            create_composite_filter_type_kind(&fields, building)
        }
    }
}

const OPERATORS: [&str; 3] = ["eq", "lt", "gt"]; // TODO: Expand and make specific for the operand types

fn create_operator_filter_type_kind(
    scalar_model_type: &ModelType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    // TODO: Create scalar_type specific filter. For example, "like" only for String
    // [eq: <scalar_type>, lt: <scalar_type>, ...]
    let parameters: Vec<_> = OPERATORS
        .iter()
        .map(|operator| PredicateParameter {
            name: operator.to_string(),
            type_name: scalar_model_type.name.to_string(),
            type_id: building
                .predicate_types
                .get_id(&scalar_model_type.name)
                .unwrap(),
            type_modifier: ModelTypeModifier::Optional,
            column_id: None,
        })
        .collect();

    PredicateParameterTypeKind::Opeartor(parameters)
}

fn create_composite_filter_type_kind(
    fields: &Vec<ModelField>,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    let parameters = fields
        .iter()
        .map(|field| {
            let param_type_name = get_parameter_type_name(&field.type_name);
            PredicateParameter {
                name: field.name.to_string(),
                type_name: param_type_name.clone(),
                type_id: building.predicate_types.get_id(&param_type_name).unwrap(),
                type_modifier: ModelTypeModifier::Optional,
                column_id: field.relation.self_column(),
            }
        })
        .collect();

    PredicateParameterTypeKind::Composite(parameters)
}
