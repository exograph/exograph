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
    types::FieldType,
};

use exo_sql::{ColumnPathLink, Database};
use postgres_model::{
    access::Access,
    order::OrderByParameter,
    order::{OrderByParameterType, OrderByParameterTypeKind, OrderByParameterTypeWrapper},
    types::{EntityType, PostgresField, PostgresPrimitiveType, PostgresType},
};

use crate::shallow::Shallow;

use super::{
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
    type_builder::ResolvedTypeEnv,
};

impl Shallow for OrderByParameter {
    fn shallow() -> Self {
        Self {
            name: String::default(),
            typ: FieldType::Plain(OrderByParameterTypeWrapper::shallow()),
            column_path_link: None,
            access: None,
        }
    }
}

impl Shallow for OrderByParameterTypeWrapper {
    fn shallow() -> Self {
        Self {
            name: String::default(),
            type_id: SerializableSlabIndex::shallow(),
        }
    }
}

pub fn build_shallow(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    let type_name = "Ordering".to_string();
    let primitive_type = OrderByParameterType {
        name: type_name.to_owned(),
        kind: OrderByParameterTypeKind::Primitive,
    };

    building.order_by_types.add(&type_name, primitive_type);

    for (_, typ) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(ResolvedCompositeType { .. }) = typ {
            let shallow_type = create_shallow_type(typ);
            let param_type_name = shallow_type.name.clone();
            building.order_by_types.add(&param_type_name, shallow_type);
        }
    }
}

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (_, entity_type) in building.entity_types.iter() {
        let param_type_name = get_parameter_type_name(&entity_type.name, false);
        let existing_param_id = building.order_by_types.get_id(&param_type_name);

        if let Some(existing_param_id) = existing_param_id {
            let new_kind = expand_type(entity_type, building);
            building.order_by_types[existing_param_id].kind = new_kind;
        }
    }
}

fn get_parameter_type_name(entity_type_name: &str, is_primitive: bool) -> String {
    if is_primitive {
        "Ordering".to_string()
    } else {
        format!("{}Ordering", &entity_type_name)
    }
}

fn create_shallow_type(typ: &ResolvedType) -> OrderByParameterType {
    OrderByParameterType {
        name: match &typ {
            ResolvedType::Primitive(p) => get_parameter_type_name(&p.name(), true),
            ResolvedType::Composite(c) => get_parameter_type_name(c.name.as_str(), false),
        },
        kind: OrderByParameterTypeKind::Composite { parameters: vec![] },
    }
}

fn expand_type(
    entity_type: &EntityType,
    building: &SystemContextBuilding,
) -> OrderByParameterTypeKind {
    let parameters = entity_type
        .fields
        .iter()
        .flat_map(|field| {
            new_field_param(
                field,
                &building.primitive_types,
                &building.entity_types,
                &building.order_by_types,
                &building.database,
            )
        })
        .collect();

    OrderByParameterTypeKind::Composite { parameters }
}

fn new_param(
    name: &str,
    entity_type_name: &str,
    is_primitive: bool,
    column_path_link: Option<ColumnPathLink>,
    order_by_types: &MappedArena<OrderByParameterType>,
    access: Option<Access>,
) -> OrderByParameter {
    let (param_type_name, param_type_id) =
        order_by_param_type(entity_type_name, is_primitive, order_by_types);

    let base_param = FieldType::Plain(OrderByParameterTypeWrapper {
        name: param_type_name,
        type_id: param_type_id,
    });

    let wrapped_type = if is_primitive {
        base_param
    } else {
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
        FieldType::List(Box::new(base_param))
    };

    OrderByParameter {
        name: name.to_string(),
        typ: FieldType::Optional(Box::new(wrapped_type)),
        column_path_link,
        access,
    }
}

pub fn new_field_param(
    entity_field: &PostgresField<EntityType>,
    primitive_types: &MappedArena<PostgresPrimitiveType>,
    entity_types: &MappedArena<EntityType>,
    order_by_types: &MappedArena<OrderByParameterType>,
    database: &Database,
) -> Option<OrderByParameter> {
    // If the field has one-to-many relationship, we cannot order by it. For example, it doesn't
    // make sense to order venues by concert id (assuming venue hosts multiple concerts)
    match &entity_field.typ {
        FieldType::List(_) => return None,
        FieldType::Optional(inner) => {
            if matches!(inner.as_ref(), FieldType::List(_)) {
                return None;
            }
        }
        FieldType::Plain(_) => (),
    }

    let field_type_id = &entity_field.typ.innermost().type_id;
    let field_entity_type =
        field_type_id.to_type(primitive_types.values_ref(), entity_types.values_ref());

    let column_path_link = Some(entity_field.relation.column_path_link(database));

    Some(new_param(
        &entity_field.name,
        field_entity_type.name(),
        matches!(field_entity_type, PostgresType::Primitive(_)),
        column_path_link,
        order_by_types,
        Some(entity_field.access.clone()),
    ))
}

pub fn new_root_param(
    entity_type_name: &str,
    is_primitive: bool,
    order_by_types: &MappedArena<OrderByParameterType>,
) -> OrderByParameter {
    new_param(
        "orderBy",
        entity_type_name,
        is_primitive,
        None,
        order_by_types,
        None,
    )
}

fn order_by_param_type(
    entity_type_name: &str,
    is_primitive: bool,
    order_by_types: &MappedArena<OrderByParameterType>,
) -> (String, SerializableSlabIndex<OrderByParameterType>) {
    let param_type_name = get_parameter_type_name(entity_type_name, is_primitive);

    let param_type_id = order_by_types.get_id(&param_type_name).unwrap();

    (param_type_name, param_type_id)
}
