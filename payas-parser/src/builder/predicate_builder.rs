use payas_model::model::{
    mapped_arena::MappedArena,
    types::{GqlField, GqlType, GqlTypeKind, GqlTypeModifier},
    GqlCompositeTypeKind,
};
use std::collections::HashMap;

use super::{
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
};
use payas_model::model::predicate::*;

use lazy_static::lazy_static;

pub fn build_shallow(models: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    for (_, model) in models.iter() {
        match model {
            ResolvedType::Primitive(pt) => {
                let type_name = pt.name();
                // One for queries such as {id: 1}, where the type name is the same as the model type name (in this case `Int`)
                building.predicate_types.add(
                    &type_name,
                    PredicateParameterType {
                        name: type_name.to_string(),
                        kind: PredicateParameterTypeKind::ImplicitEqual {},
                    },
                );

                // Another one for operators
                let param_type_name = get_parameter_type_name(&type_name);
                building.predicate_types.add(
                    &param_type_name,
                    PredicateParameterType {
                        name: param_type_name.to_string(),
                        kind: PredicateParameterTypeKind::ImplicitEqual {},
                    },
                );
            }
            ResolvedType::Composite(c) => {
                let shallow_type = create_shallow_type(c);
                let param_type_name = shallow_type.name.clone();
                building.predicate_types.add(&param_type_name, shallow_type);
            }
        }
    }
}

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (_, model_type) in building.types.iter() {
        let param_type_name = get_parameter_type_name(&model_type.name);
        let existing_param_id = building.predicate_types.get_id(&param_type_name);

        let new_kind = expand_type(model_type, building);
        building.predicate_types[existing_param_id.unwrap()].kind = new_kind;
    }
}

pub fn get_parameter_type_name(model_type_name: &str) -> String {
    format!("{}Filter", model_type_name)
}

fn create_shallow_type(model: &ResolvedCompositeType) -> PredicateParameterType {
    PredicateParameterType {
        name: get_parameter_type_name(model.name.as_str()),
        kind: PredicateParameterTypeKind::ImplicitEqual, // Will be set to the correct value in expand_type
    }
}

fn expand_type(gql_type: &GqlType, building: &SystemContextBuilding) -> PredicateParameterTypeKind {
    match &gql_type.kind {
        GqlTypeKind::Primitive => create_operator_filter_type_kind(gql_type, building),
        GqlTypeKind::Composite(GqlCompositeTypeKind { fields, .. }) => {
            create_composite_filter_type_kind(fields, building)
        }
    }
}

lazy_static! {
    // immutable map defining the operators allowed for each type
    // TODO: could probably be done better?
    static ref TYPE_OPERATORS: HashMap<&'static str, Option<Vec<&'static str>>> = {
        let mut supported_operators = HashMap::new();

        supported_operators.insert(
            "Int",
            Some(vec![
                "eq", "neq",
                "lt", "lte", "gt", "gte"
            ])
        );

        supported_operators.insert(
            "String",
            Some(vec![
                "eq", "neq",
                "lt", "lte", "gt", "gte",
                "like", "startsWith", "endsWith"
            ])
        );

        supported_operators.insert(
            "Boolean",
            Some(vec!["eq", "neq"])
        );

        let datetime_operators = Some(vec![
            "eq", "neq",
            "lt", "lte", "gt", "gte"
        ]);

        supported_operators.insert("LocalTime", datetime_operators.clone());
        supported_operators.insert("LocalDateTime", datetime_operators.clone());
        supported_operators.insert("LocalDate", datetime_operators.clone());
        supported_operators.insert("Instant", datetime_operators.clone());

        supported_operators.insert(
            "Json",
            Some(vec!["contains", "containedBy", "matchKey", "matchAllKeys", "matchAnyKey"])
        );

        supported_operators
    };
}

fn create_operator_filter_type_kind(
    scalar_model_type: &GqlType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    let parameter_constructor = |operator: &&str| PredicateParameter {
        name: operator.to_string(),
        type_name: scalar_model_type.name.to_string(),
        type_id: building
            .predicate_types
            .get_id(&scalar_model_type.name)
            .unwrap(),
        type_modifier: GqlTypeModifier::Optional,
        column_id: None,
    };

    // look up type in (type, operations) table
    if let Some(maybe_operators) = TYPE_OPERATORS.get(&scalar_model_type.name as &str) {
        if let Some(operators) = maybe_operators {
            // type supports specific operations, construct kind with supported operations
            let parameters: Vec<PredicateParameter> =
                operators.iter().map(parameter_constructor).collect();

            PredicateParameterTypeKind::Opeartor(parameters)
        } else {
            // type supports no specific operations, assume implicit equals
            PredicateParameterTypeKind::ImplicitEqual
        }
    } else {
        todo!("{} does not support any operators", scalar_model_type.name)
    } // type given is not listed in TYPE_OPERATORS?
}

fn create_composite_filter_type_kind(
    fields: &[GqlField],
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    let parameters = fields
        .iter()
        .map(|field| {
            let param_type_name = get_parameter_type_name(field.typ.type_name());
            PredicateParameter {
                name: field.name.to_string(),
                type_name: param_type_name.clone(),
                type_id: building.predicate_types.get_id(&param_type_name).unwrap(),
                type_modifier: GqlTypeModifier::Optional,
                column_id: field.relation.self_column(),
            }
        })
        .collect();

    PredicateParameterTypeKind::Composite(parameters)
}
