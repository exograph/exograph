// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::{
    mapped_arena::{MappedArena, SerializableSlabIndex},
    types::{FieldType, Named},
};
use exo_sql::ColumnPathLink;
use postgres_core_model::types::{EntityType, PostgresField, PostgresPrimitiveType, TypeIndex};
use postgres_core_model::{
    predicate::{
        PredicateParameter, PredicateParameterType, PredicateParameterTypeKind,
        PredicateParameterTypeWrapper,
    },
    types::PostgresPrimitiveTypeKind,
};
use postgres_core_model::{relation::PostgresRelation, types::EntityRepresentation};

use super::system_builder::SystemContextBuilding;

use crate::shallow::Shallow;
use crate::type_provider::PRIMITIVE_TYPE_PROVIDER_REGISTRY;
use crate::{
    resolved_type::{ResolvedCompositeType, ResolvedType, ResolvedTypeEnv},
    type_provider::VectorTypeHint,
};

impl Shallow for PredicateParameter {
    fn shallow() -> Self {
        Self {
            name: String::new(),
            typ: FieldType::Plain(PredicateParameterTypeWrapper::shallow()),
            column_path_link: None,
            access: None,
            vector_distance_function: None,
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
                        kind: PredicateParameterTypeKind::ImplicitEqual,
                        underlying_type: None,
                    },
                );

                // Another one for operators
                let param_type_name = get_filter_type_name(&type_name); // For example, IntFilter
                building.predicate_types.add(
                    &param_type_name,
                    PredicateParameterType {
                        name: param_type_name.to_string(),
                        kind: PredicateParameterTypeKind::ImplicitEqual,
                        underlying_type: None,
                    },
                );
            }
            ResolvedType::Enum(e) => {
                building.predicate_types.add(
                    &e.name,
                    PredicateParameterType {
                        name: e.name.to_string(),
                        kind: PredicateParameterTypeKind::ImplicitEqual,
                        underlying_type: None,
                    },
                );

                let type_name = get_filter_type_name(&e.name);
                building.predicate_types.add(
                    &type_name,
                    PredicateParameterType {
                        name: type_name.to_string(),
                        kind: PredicateParameterTypeKind::ImplicitEqual,
                        underlying_type: None,
                    },
                );
            }
            ResolvedType::Composite(c @ ResolvedCompositeType { .. }) => {
                if c.representation == EntityRepresentation::Json {
                    continue;
                }

                // Generic filter type
                {
                    let shallow_type = PredicateParameterType {
                        name: get_filter_type_name(&c.name),
                        kind: PredicateParameterTypeKind::ImplicitEqual, // Will be set to the correct value in expand_type
                        underlying_type: None,
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
                        underlying_type: None,
                    };
                    building
                        .predicate_types
                        .add(&shallow_type.name.clone(), shallow_type);
                }
            }
        }
    }
    building.predicate_types.add(
        "VectorFilterArg",
        PredicateParameterType {
            name: "VectorFilterArg".to_string(),
            kind: PredicateParameterTypeKind::Vector,
            underlying_type: None,
        },
    );
}

pub fn build_expanded(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    for (_, primitive_type) in building.primitive_types.iter() {
        let param_type_name = get_filter_type_name(&primitive_type.name);
        let existing_param_id = building.predicate_types.get_id(&param_type_name);

        let new_kind = expand_primitive_type(primitive_type, building);
        building.predicate_types[existing_param_id.unwrap()].kind = new_kind;
    }

    for (entity_type_id, entity_type) in building.entity_types.iter() {
        if entity_type.representation == EntityRepresentation::Json {
            continue;
        }

        {
            let param_type_name = get_filter_type_name(&entity_type.name);
            let existing_param_id = building.predicate_types.get_id(&param_type_name);

            let resolved_type = resolved_env
                .resolved_types
                .get_by_key(&entity_type.name)
                .unwrap();

            let new_kind = expand_entity_type(resolved_type, entity_type, building);
            let param_type = &mut building.predicate_types[existing_param_id.unwrap()];
            param_type.kind = new_kind;
            param_type.underlying_type = Some(entity_type_id);
        }

        {
            let param_type_name = get_unique_filter_type_name(&entity_type.name);
            let existing_param_id = building.predicate_types.get_id(&param_type_name);
            let new_kind = expand_unique_type(entity_type, building);

            let param_type = &mut building.predicate_types[existing_param_id.unwrap()];
            param_type.kind = new_kind;
            param_type.underlying_type = Some(entity_type_id);
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
    if matches!(typ.kind, PostgresPrimitiveTypeKind::Enum(_)) {
        expand_enum_type(typ, building)
    } else {
        create_operator_filter_type_kind(typ, building)
    }
}

fn expand_enum_type(
    typ: &PostgresPrimitiveType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    let param_type_id = building.predicate_types.get_id(&typ.name).unwrap();

    let parameters: Vec<PredicateParameter> = ["eq", "neq"]
        .iter()
        .map(|operator| PredicateParameter {
            name: operator.to_string(),
            typ: FieldType::Optional(Box::new(FieldType::Plain(PredicateParameterTypeWrapper {
                name: typ.name.to_owned(),
                type_id: param_type_id,
            }))),
            column_path_link: None,
            access: None,
            vector_distance_function: None,
        })
        .collect();

    PredicateParameterTypeKind::Operator(parameters)
}

fn expand_entity_type(
    resolved_type: &ResolvedType,
    entity_type: &EntityType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    if entity_type.representation == EntityRepresentation::Json {
        return PredicateParameterTypeKind::ImplicitEqual;
    }

    fn is_normal_field(
        field: &PostgresField<EntityType>,
        building: &SystemContextBuilding,
    ) -> bool {
        let field_type_id = &field.typ.innermost().type_id;
        if let TypeIndex::Composite(index) = field_type_id {
            let field_type = &building.entity_types[*index];
            field_type.representation != EntityRepresentation::Json
        } else {
            true
        }
    }

    let entity_type_name = &entity_type.name;
    // populate params for each field
    let field_params: Vec<PredicateParameter> = entity_type
        .fields
        .iter()
        .filter(|field| is_normal_field(field, building))
        .map(|field| {
            let param_type_name = get_filter_type_name(field.typ.name());

            let column_path_link = Some(field.relation.column_path_link(&building.database));

            let resolved_field = resolved_type
                .as_composite()
                .fields
                .iter()
                .find(|f| f.name == field.name)
                .unwrap();

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
                vector_distance_function: resolved_field.type_hint.as_ref().and_then(|hint| {
                    (hint.0.as_ref() as &dyn std::any::Any)
                        .downcast_ref::<VectorTypeHint>()
                        .and_then(|v| v.distance_function)
                }),
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
                vector_distance_function: None,
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
            PostgresRelation::Scalar {
                is_pk: true,
                column_id,
            } => {
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
                    column_path_link: Some(ColumnPathLink::Leaf(*column_id)),
                    vector_distance_function: None,
                })
            }
            PostgresRelation::ManyToOne { is_pk: true, .. } => {
                let field_type_name = field.typ.name();
                let param_type_id = building
                    .predicate_types
                    .get_id(&get_unique_filter_type_name(field_type_name))
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
                    vector_distance_function: None,
                })
            }
            _ => None,
        })
        .collect();

    PredicateParameterTypeKind::Reference(field_predicates)
}

fn create_operator_filter_type_kind(
    primitive_type: &PostgresPrimitiveType,
    building: &SystemContextBuilding,
) -> PredicateParameterTypeKind {
    let parameter_constructor = |operator: &&str| {
        // For Vector's similar operation, we need to use the VectorFilterArg type (which has two fields: value and distance)
        let operand_type = if operator == &"similar" && primitive_type.name == "Vector" {
            "VectorFilterArg"
        } else {
            primitive_type.name.as_str()
        };
        let predicate_param_type_id = building.predicate_types.get_id(operand_type).unwrap();

        PredicateParameter {
            name: operator.to_string(),
            typ: FieldType::Optional(Box::new(FieldType::Plain(PredicateParameterTypeWrapper {
                name: operand_type.to_owned(),
                type_id: predicate_param_type_id,
            }))),
            column_path_link: None,
            access: None,
            vector_distance_function: None,
        }
    };

    // look up type provider to get supported operators
    if let Some(type_provider) = PRIMITIVE_TYPE_PROVIDER_REGISTRY.get(primitive_type.name.as_str())
    {
        if let Some(operators) = type_provider.supported_operators() {
            // type supports specific operations, construct kind with supported operations
            let parameters: Vec<PredicateParameter> =
                operators.iter().map(parameter_constructor).collect();

            PredicateParameterTypeKind::Operator(parameters)
        } else {
            // type supports no specific operations, assume implicit equals
            PredicateParameterTypeKind::ImplicitEqual
        }
    } else {
        // type given is not listed in PRIMITIVE_TYPE_PROVIDER_REGISTRY?
        todo!(
            "Unknown type: {}. This should not happen.",
            primitive_type.name
        )
    }
}
