use payas_model::model::{
    mapped_arena::{MappedArena, SerializableSlabIndex},
    order::OrderByParameter,
    order::{OrderByParameterType, OrderByParameterTypeKind, OrderByParameterTypeWithModifier},
    predicate::ColumnIdPathLink,
    types::{GqlCompositeType, GqlField, GqlType, GqlTypeKind, GqlTypeModifier},
};

use payas_core_model_builder::builder::{
    column_path_utils,
    resolved_builder::{ResolvedCompositeType, ResolvedCompositeTypeKind, ResolvedType},
    type_builder::ResolvedTypeEnv,
};

use super::system_builder::SystemContextBuilding;

pub fn build_shallow(models: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    let type_name = "Ordering".to_string();
    let primitive_type = OrderByParameterType {
        name: type_name.to_owned(),
        kind: OrderByParameterTypeKind::Primitive,
    };

    building.order_by_types.add(&type_name, primitive_type);

    for (_, model) in models.iter() {
        if let ResolvedType::Composite(ResolvedCompositeType {
            kind: ResolvedCompositeTypeKind::Persistent { .. },
            ..
        }) = model
        {
            let shallow_type = create_shallow_type(model);
            let param_type_name = shallow_type.name.clone();
            building.order_by_types.add(&param_type_name, shallow_type);
        }
    }
}

pub fn build_expanded(env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    for (_, model_type) in env
        .base_system
        .primitive_types
        .iter()
        .chain(building.database_types.iter())
    {
        let param_type_name = get_parameter_type_name(&model_type.name, model_type.is_primitive());
        let existing_param_id = building.order_by_types.get_id(&param_type_name);

        if let Some(existing_param_id) = existing_param_id {
            let new_kind = expand_type(model_type, env, building);
            building.order_by_types[existing_param_id].kind = new_kind;
        }
    }
}

pub fn get_parameter_type_name(model_type_name: &str, is_primitive: bool) -> String {
    if is_primitive {
        "Ordering".to_string()
    } else {
        format!("{}Ordering", &model_type_name)
    }
}

fn create_shallow_type(model: &ResolvedType) -> OrderByParameterType {
    OrderByParameterType {
        name: match &model {
            ResolvedType::Primitive(p) => get_parameter_type_name(&p.name(), true),
            ResolvedType::Composite(c) => get_parameter_type_name(c.name.as_str(), false),
        },
        kind: OrderByParameterTypeKind::Composite { parameters: vec![] },
    }
}

fn expand_type(
    model_type: &GqlType,
    env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> OrderByParameterTypeKind {
    match &model_type.kind {
        GqlTypeKind::Primitive => OrderByParameterTypeKind::Primitive,
        GqlTypeKind::Composite(composite_type @ GqlCompositeType { fields, .. }) => {
            let parameters = fields
                .iter()
                .map(|field| new_field_param(field, composite_type, env, building))
                .collect();

            OrderByParameterTypeKind::Composite { parameters }
        }
    }
}

fn new_param(
    name: &str,
    model_type_name: &str,
    is_primitive: bool,
    column_path_link: Option<ColumnIdPathLink>,
    building: &SystemContextBuilding,
) -> OrderByParameter {
    let (param_type_name, param_type_id) =
        order_by_param_type(model_type_name, is_primitive, building);

    OrderByParameter {
        name: name.to_string(),
        type_name: param_type_name,
        typ: OrderByParameterTypeWithModifier {
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
            type_modifier: GqlTypeModifier::List,
        },
        column_path_link,
    }
}

pub fn new_field_param(
    model_field: &GqlField,
    composite_type: &GqlCompositeType,
    env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> OrderByParameter {
    let is_field_primitive = model_field.typ.is_primitive();
    let field_type_id = model_field.typ.type_id().to_owned();
    let field_model_type = if is_field_primitive {
        &env.base_system.primitive_types[field_type_id]
    } else {
        &building.database_types[field_type_id]
    };

    let column_path_link = Some(column_path_utils::column_path_link(
        composite_type,
        model_field,
        env,
        &building.database_types,
    ));

    new_param(
        &model_field.name,
        &field_model_type.name,
        field_model_type.is_primitive(),
        column_path_link,
        building,
    )
}

pub fn new_root_param(
    model_type_name: &str,
    is_primitive: bool,
    building: &SystemContextBuilding,
) -> OrderByParameter {
    new_param("orderBy", model_type_name, is_primitive, None, building)
}

fn order_by_param_type(
    model_type_name: &str,
    is_primitive: bool,
    building: &SystemContextBuilding,
) -> (String, SerializableSlabIndex<OrderByParameterType>) {
    let param_type_name = get_parameter_type_name(model_type_name, is_primitive);

    let param_type_id = building.order_by_types.get_id(&param_type_name).unwrap();

    (param_type_name, param_type_id)
}
