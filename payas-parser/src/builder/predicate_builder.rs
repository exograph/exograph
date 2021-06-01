use payas_model::model::types::{ModelField, ModelType, ModelTypeKind, ModelTypeModifier};

use super::{
    system_builder::SystemContextBuilding, type_builder::PRIMITIVE_TYPE_NAMES, typechecking::Type,
};
use payas_model::model::predicate::*;

pub fn build_shallow(ast_types: &[Type], building: &mut SystemContextBuilding) {
    for type_name in PRIMITIVE_TYPE_NAMES.iter() {
        // One for queries such as {id: 1}, where the type name is the same as the model type name (in this case `Int`)
        building.predicate_types.add(
            type_name,
            PredicateParameterType {
                name: type_name.to_string(),
                kind: PredicateParameterTypeKind::ImplicitEqual {},
            },
        );

        // Another one for operators
        let param_type_name = get_parameter_type_name(type_name);
        building.predicate_types.add(
            &param_type_name,
            PredicateParameterType {
                name: param_type_name.to_string(),
                kind: PredicateParameterTypeKind::ImplicitEqual {},
            },
        );
    }

    for ast_type in ast_types.iter() {
        let shallow_type = create_shallow_type(ast_type);
        let param_type_name = shallow_type.name.clone();
        building.predicate_types.add(&param_type_name, shallow_type);
    }
}

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (_, model_type) in building.types.iter() {
        let param_type_name = get_parameter_type_name(&model_type.name);
        let existing_param_id = building.predicate_types.get_id(&param_type_name);

        let new_kind = expand_type(&model_type, building);
        building.predicate_types[existing_param_id.unwrap()].kind = new_kind;
    }
}

pub fn get_parameter_type_name(model_type_name: &str) -> String {
    format!("{}Filter", model_type_name)
}

fn create_shallow_type(ast_type: &Type) -> PredicateParameterType {
    PredicateParameterType {
        name: get_parameter_type_name(&ast_type.UNSAFE_name()),
        kind: PredicateParameterTypeKind::ImplicitEqual, // Will be set to the correct value in expand_type
    }
}

fn expand_type(
    model_type: &ModelType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    match &model_type.kind {
        ModelTypeKind::Primitive => create_operator_filter_type_kind(model_type, building),
        ModelTypeKind::Composite { fields, .. } => {
            create_composite_filter_type_kind(fields, building)
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
    let parameters = OPERATORS
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
    fields: &[ModelField],
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    let parameters = fields
        .iter()
        .map(|field| {
            let param_type_name = get_parameter_type_name(&field.typ.type_name());
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
