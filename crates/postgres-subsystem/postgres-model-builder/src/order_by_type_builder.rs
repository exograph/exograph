use core_plugin_interface::core_model::{mapped_arena::SerializableSlabIndex, types::FieldType};

use postgres_model::{
    column_path::ColumnIdPathLink,
    order::OrderByParameter,
    order::{OrderByParameterType, OrderByParameterTypeKind, OrderByParameterTypeWrapper},
    types::{EntityType, PostgresField, PostgresType},
};

use crate::shallow::Shallow;

use super::{
    column_path_utils,
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
        .map(|field| new_field_param(field, entity_type, building))
        .collect();

    OrderByParameterTypeKind::Composite { parameters }
}

fn new_param(
    name: &str,
    entity_type_name: &str,
    is_primitive: bool,
    column_path_link: Option<ColumnIdPathLink>,
    building: &SystemContextBuilding,
) -> OrderByParameter {
    let (param_type_name, param_type_id) =
        order_by_param_type(entity_type_name, is_primitive, building);

    OrderByParameter {
        name: name.to_string(),

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
        typ: FieldType::Optional(Box::new(FieldType::List(Box::new(FieldType::Plain(
            OrderByParameterTypeWrapper {
                name: param_type_name,
                type_id: param_type_id,
            },
        ))))),
        column_path_link,
    }
}

pub fn new_field_param(
    entity_field: &PostgresField<EntityType>,
    composite_type: &EntityType,
    building: &SystemContextBuilding,
) -> OrderByParameter {
    let field_type_id = entity_field.typ.inner_most().type_id.to_owned();
    let field_entity_type = field_type_id.to_type(
        &building.primitive_types.values,
        &building.entity_types.values,
    );

    let column_path_link = Some(column_path_utils::column_path_link(
        composite_type,
        entity_field,
        &building.entity_types,
    ));

    new_param(
        &entity_field.name,
        field_entity_type.name(),
        matches!(field_entity_type, PostgresType::Primitive(_)),
        column_path_link,
        building,
    )
}

pub fn new_root_param(
    entity_type_name: &str,
    is_primitive: bool,
    building: &SystemContextBuilding,
) -> OrderByParameter {
    new_param("orderBy", entity_type_name, is_primitive, None, building)
}

fn order_by_param_type(
    entity_type_name: &str,
    is_primitive: bool,
    building: &SystemContextBuilding,
) -> (String, SerializableSlabIndex<OrderByParameterType>) {
    let param_type_name = get_parameter_type_name(entity_type_name, is_primitive);

    let param_type_id = building.order_by_types.get_id(&param_type_name).unwrap();

    (param_type_name, param_type_id)
}
