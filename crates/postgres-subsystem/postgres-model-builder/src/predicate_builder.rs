// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::{
    mapped_arena::{MappedArena, SerializableSlabIndex},
    types::{FieldType, Named},
};
use postgres_model::{
    predicate::PredicateParameterTypeWrapper,
    relation::PostgresRelation,
    types::{EntityType, PostgresPrimitiveType},
};
use std::collections::HashMap;

use crate::shallow::Shallow;

use super::{
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
            typ: FieldType::Plain(PredicateParameterTypeWrapper::shallow()),
            column_path_link: None,
            access: None,
        }
    }
}

impl Shallow for PredicateParameterTypeWrapper {
    fn shallow() -> Self {
        Self {
            name: String::new(),
            type_id: SerializableSlabIndex::shallow(),
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
                let param_type_name = get_filter_type_name(&type_name); // For example, IntFilter
                building.predicate_types.add(
                    &param_type_name,
                    PredicateParameterType {
                        name: param_type_name.to_string(),
                        kind: PredicateParameterTypeKind::ImplicitEqual {},
                    },
                );
            }
            ResolvedType::Composite(c @ ResolvedCompositeType { .. }) => {
                // Generic filter type
                {
                    let shallow_type = PredicateParameterType {
                        name: get_filter_type_name(&c.name),
                        kind: PredicateParameterTypeKind::ImplicitEqual, // Will be set to the correct value in expand_type
                    };
                    building
                        .predicate_types
                        .add(&shallow_type.name.clone(), shallow_type);
                }
                // Unique filter type
                {
                    let shallow_type = PredicateParameterType {
                        name: get_unique_filter_type_name(&c.name),
                        kind: PredicateParameterTypeKind::ImplicitEqual, // Will be set to the correct value in expand_type
                    };
                    building
                        .predicate_types
                        .add(&shallow_type.name.clone(), shallow_type);
                }
            }
        }
    }
}

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (_, primitive_type) in building.primitive_types.iter() {
        let param_type_name = get_filter_type_name(&primitive_type.name);
        let existing_param_id = building.predicate_types.get_id(&param_type_name);

        let new_kind = expand_primitive_type(primitive_type, building);
        building.predicate_types[existing_param_id.unwrap()].kind = new_kind;
    }

    for (_, entity_type) in building.entity_types.iter() {
        {
            let param_type_name = get_filter_type_name(&entity_type.name);
            let existing_param_id = building.predicate_types.get_id(&param_type_name);

            let new_kind = expand_entity_type(entity_type, building);
            building.predicate_types[existing_param_id.unwrap()].kind = new_kind;
        }

        {
            let param_type_name = get_unique_filter_type_name(&entity_type.name);
            let existing_param_id = building.predicate_types.get_id(&param_type_name);
            let new_kind = expand_unique_type(entity_type, building);
            building.predicate_types[existing_param_id.unwrap()].kind = new_kind;
        }
    }
}

pub fn get_filter_type_name(type_name: &str) -> String {
    format!("{type_name}Filter")
}

pub fn get_unique_filter_type_name(type_name: &str) -> String {
    format!("{type_name}UniqueFilter")
}

fn expand_primitive_type(
    typ: &PostgresPrimitiveType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    create_operator_filter_type_kind(typ, building)
}

fn expand_entity_type(
    entity_type: &EntityType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    let entity_type_name = &entity_type.name;
    // populate params for each field
    let field_params: Vec<PredicateParameter> = entity_type
        .fields
        .iter()
        .map(|field| {
            let param_type_name = get_filter_type_name(field.typ.name());

            let column_path_link = Some(field.relation.column_path_link(&building.database));

            PredicateParameter {
                name: field.name.to_string(),
                typ: FieldType::Optional(Box::new(FieldType::Plain(
                    PredicateParameterTypeWrapper {
                        type_id: building.predicate_types.get_id(&param_type_name).unwrap(),
                        name: param_type_name,
                    },
                ))),
                column_path_link,
                access: Some(field.access.clone()),
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
            let param_type_name = get_filter_type_name(entity_type_name);
            let param_type_id = building
                .predicate_types
                .get_id(&param_type_name)
                .unwrap_or_else(|| panic!("Could not find predicate type '{param_type_name}'"));
            let param_type = FieldType::Plain(PredicateParameterTypeWrapper {
                name: param_type_name,
                type_id: param_type_id,
            });

            let param_field_type = if type_modifier == LogicalOpModifier::Optional {
                FieldType::Optional(Box::new(param_type))
            } else {
                FieldType::Optional(Box::new(FieldType::List(Box::new(param_type))))
            };
            PredicateParameter {
                name: name.to_string(),
                typ: param_field_type,
                column_path_link: None,
                access: None,
            }
        })
        .collect();

    PredicateParameterTypeKind::Composite {
        field_params,
        logical_op_params,
    }
}

fn expand_unique_type(
    entity_type: &EntityType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    let field_predicates = entity_type
        .fields
        .iter()
        .flat_map(|field| match &field.relation {
            PostgresRelation::Pk { .. } => {
                let field_type_name = field.typ.name();
                let param_type_id = building
                    .predicate_types
                    .get_id(field_type_name)
                    .unwrap_or_else(|| panic!("Could not find predicate type '{field_type_name}'"));

                let param_type = FieldType::Plain(PredicateParameterTypeWrapper {
                    name: field_type_name.to_owned(),
                    type_id: param_type_id,
                });

                Some(PredicateParameter {
                    name: field.name.clone(),
                    typ: param_type,
                    access: Some(field.access.clone()),
                    column_path_link: None,
                })
            }
            _ => None,
        })
        .collect();

    PredicateParameterTypeKind::Reference(field_predicates)
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

        supported_operators.insert("Vector", Some(vec!["similar"]));

        supported_operators.insert("Exograph", None);
        supported_operators.insert("ExographPriv", None);
        supported_operators.insert("Operation", None); // TODO: Re-examine if this is the best way (for both injected and interception)

        supported_operators
    };
}

fn create_operator_filter_type_kind(
    primitive_type: &PostgresPrimitiveType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    let parameter_constructor = |operator: &&str| {
        let predicate_param_type_id = building
            .predicate_types
            .get_id(&primitive_type.name)
            .unwrap();

        PredicateParameter {
            name: operator.to_string(),
            typ: FieldType::Optional(Box::new(FieldType::Plain(PredicateParameterTypeWrapper {
                name: primitive_type.name.to_owned(),
                type_id: predicate_param_type_id,
            }))),
            column_path_link: None,
            access: None,
        }
    };

    // look up type in (type, operations) table
    if let Some(maybe_operators) = TYPE_OPERATORS.get(&primitive_type.name as &str) {
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
        todo!("{} does not support any operators", primitive_type.name)
    } // type given is not listed in TYPE_OPERATORS?
}
