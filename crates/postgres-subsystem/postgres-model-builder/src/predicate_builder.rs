use core_plugin_interface::core_model::{
    mapped_arena::{MappedArena, SerializableSlabIndex},
    types::{FieldType, Named},
};
use postgres_model::types::{EntityType, PostgresPrimitiveType, TypeIndex};
use std::collections::HashMap;

use crate::shallow::Shallow;

use super::{
    column_path_utils,
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
};
use postgres_model::predicate::{
    PredicateParameter, PredicateParameterType, PredicateParameterTypeKind,
};

use lazy_static::lazy_static;

impl Shallow for PredicateParameter {
    fn shallow() -> Self {
        Self {
            name: String::new(),
            type_name: String::new(),
            type_id: SerializableSlabIndex::shallow(),
            typ: FieldType::Plain(PredicateParameterType::shallow()),
            column_path_link: None,
            underlying_type_id: TypeIndex::shallow(),
        }
    }
}

impl Shallow for PredicateParameterType {
    fn shallow() -> Self {
        Self {
            name: String::new(),
            kind: PredicateParameterTypeKind::ImplicitEqual,
        }
    }
}

pub fn build_shallow(types: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    for (_, typ) in types.iter() {
        match typ {
            ResolvedType::Primitive(pt) => {
                let type_name = pt.name();
                // One for queries such as {id: 1}, where the type name is the same as the type name (in this case `Int`)
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
            ResolvedType::Composite(c @ ResolvedCompositeType { .. }) => {
                let shallow_type = create_shallow_type(c);
                let param_type_name = shallow_type.name.clone();
                building.predicate_types.add(&param_type_name, shallow_type);
            }
        }
    }
}

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (type_id, typ) in building.primitive_types.iter() {
        let param_type_name = get_parameter_type_name(&typ.name);
        let existing_param_id = building.predicate_types.get_id(&param_type_name);

        let new_kind = expand_primitive_type(type_id, typ, building);
        building.predicate_types[existing_param_id.unwrap()].kind = new_kind;
    }

    for (model_type_id, model_type) in building.entity_types.iter() {
        let param_type_name = get_parameter_type_name(&model_type.name);
        let existing_param_id = building.predicate_types.get_id(&param_type_name);

        let new_kind = expand_entity_type(model_type_id, model_type, building);
        building.predicate_types[existing_param_id.unwrap()].kind = new_kind;
    }
}

pub fn get_parameter_type_name(model_type_name: &str) -> String {
    format!("{model_type_name}Filter")
}

fn create_shallow_type(model: &ResolvedCompositeType) -> PredicateParameterType {
    PredicateParameterType {
        name: get_parameter_type_name(model.name.as_str()),
        kind: PredicateParameterTypeKind::ImplicitEqual, // Will be set to the correct value in expand_type
    }
}

fn expand_primitive_type(
    type_id: SerializableSlabIndex<PostgresPrimitiveType>,
    typ: &PostgresPrimitiveType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    create_operator_filter_type_kind(type_id, typ, building)
}

fn expand_entity_type(
    postgres_type_id: SerializableSlabIndex<EntityType>,
    entity_type: &EntityType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    create_composite_filter_type_kind(postgres_type_id, entity_type, &entity_type.name, building)
}

lazy_static! {
    // immutable map defining the operators allowed for each type
    // TODO: could probably be done better?
    static ref TYPE_OPERATORS: HashMap<&'static str, Option<Vec<&'static str>>> = {
        let mut supported_operators = HashMap::new();

        let numeric_operators = Some(vec![
            "eq", "neq",
            "lt", "lte", "gt", "gte"
        ]);

        supported_operators.insert("Int", numeric_operators.clone());
        supported_operators.insert("Float", numeric_operators.clone());
        supported_operators.insert("Decimal", numeric_operators.clone());

        supported_operators.insert(
            "String",
            Some(vec![
                "eq", "neq",
                "lt", "lte", "gt", "gte",
                "like", "ilike", "startsWith", "endsWith"
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

        supported_operators.insert(
            "Blob",
            None
        );

        supported_operators.insert(
            "Uuid",
            Some(vec!["eq", "neq"])
        );

        supported_operators.insert("Claytip", None);
        supported_operators.insert("ClaytipPriv", None);
        supported_operators.insert("Operation", None); // TODO: Re-examine if this is the best way (for both injected and interception)

        supported_operators
    };
}

fn create_operator_filter_type_kind(
    postgres_type_id: SerializableSlabIndex<PostgresPrimitiveType>,
    scalar_model_type: &PostgresPrimitiveType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    let parameter_constructor = |operator: &&str| {
        let predicate_param_type_id = building
            .predicate_types
            .get_id(&scalar_model_type.name)
            .unwrap();
        let predicate_param_type = building.predicate_types[predicate_param_type_id].clone();

        PredicateParameter {
            name: operator.to_string(),
            type_name: scalar_model_type.name.to_string(),
            type_id: predicate_param_type_id,
            typ: FieldType::Optional(Box::new(FieldType::Plain(predicate_param_type))),
            column_path_link: None,
            underlying_type_id: TypeIndex::Primitive(postgres_type_id),
        }
    };

    // look up type in (type, operations) table
    if let Some(maybe_operators) = TYPE_OPERATORS.get(&scalar_model_type.name as &str) {
        if let Some(operators) = maybe_operators {
            // type supports specific operations, construct kind with supported operations
            let parameters: Vec<PredicateParameter> =
                operators.iter().map(parameter_constructor).collect();

            PredicateParameterTypeKind::Operator(parameters)
        } else {
            // type supports no specific operations, assume implicit equals
            PredicateParameterTypeKind::ImplicitEqual
        }
    } else {
        todo!("{} does not support any operators", scalar_model_type.name)
    } // type given is not listed in TYPE_OPERATORS?
}

fn create_composite_filter_type_kind(
    postgres_type_id: SerializableSlabIndex<EntityType>,
    composite_type: &EntityType,
    composite_type_name: &str,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    // populate params for each field
    let field_params: Vec<PredicateParameter> = composite_type
        .fields
        .iter()
        .map(|field| {
            let param_type_name = get_parameter_type_name(field.typ.name());
            let param_type_id = &field.typ.inner_most().type_id;

            let column_path_link = Some(column_path_utils::column_path_link(
                composite_type,
                field,
                &building.entity_types,
            ));

            PredicateParameter {
                name: field.name.to_string(),
                type_name: param_type_name.clone(),
                type_id: building.predicate_types.get_id(&param_type_name).unwrap(),
                typ: FieldType::Optional(Box::new(FieldType::Plain(PredicateParameterType {
                    name: param_type_name,
                    kind: PredicateParameterTypeKind::ImplicitEqual,
                }))),
                underlying_type_id: param_type_id.clone(),
                column_path_link,
            }
        })
        .collect();

    #[derive(Debug, PartialEq, Eq)]
    enum LogicalOpModifier {
        List,     // logical op takes a list of predicates
        Optional, // logical op takes a single predicate
    }
    // populate logical ops predicate parameters
    let logical_ops = [
        ("and", LogicalOpModifier::List),
        ("or", LogicalOpModifier::List),
        ("not", LogicalOpModifier::Optional),
    ];

    let logical_op_params = logical_ops
        .into_iter()
        .map(|(name, type_modifier)| {
            let param_type_name = get_parameter_type_name(composite_type_name);
            let param_type_id = building
                .predicate_types
                .get_id(&param_type_name)
                .unwrap_or_else(|| panic!("Could not find predicate type '{param_type_name}'"));
            let param_type = FieldType::Plain(building.predicate_types[param_type_id].clone());

            let param_field_type = if type_modifier == LogicalOpModifier::Optional {
                FieldType::Optional(Box::new(param_type))
            } else {
                FieldType::Optional(Box::new(FieldType::List(Box::new(param_type))))
            };
            PredicateParameter {
                name: name.to_string(),
                type_name: get_parameter_type_name(composite_type_name),
                type_id: param_type_id,
                typ: param_field_type,
                column_path_link: None,
                underlying_type_id: TypeIndex::Composite(postgres_type_id),
            }
        })
        .collect();

    PredicateParameterTypeKind::Composite {
        field_params,
        logical_op_params,
    }
}
