use super::naming::ToGqlQueryName;
use payas_core_model_builder::builder::type_builder::ResolvedTypeEnv;
use payas_model::model::limit_offset::{LimitParameterType, OffsetParameter, OffsetParameterType};
use payas_model::model::mapped_arena::SerializableSlabIndex;
use payas_model::model::operation::{DatabaseQueryParameter, Interceptors, QueryKind};
use payas_model::model::predicate::{ColumnIdPathLink, PredicateParameterTypeWithModifier};
use payas_model::model::GqlCompositeType;
use payas_model::model::{
    limit_offset::LimitParameter,
    mapped_arena::MappedArena,
    operation::{OperationReturnType, Query},
    predicate::PredicateParameter,
    GqlType, GqlTypeKind, GqlTypeModifier,
};

use super::{order_by_type_builder, predicate_builder, system_builder::SystemContextBuilding};
use payas_core_model_builder::builder::resolved_builder::{
    ResolvedCompositeType, ResolvedCompositeTypeKind, ResolvedType,
};

pub fn build_shallow(
    models: &MappedArena<ResolvedType>,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) {
    for (_, model) in models.iter() {
        if let ResolvedType::Composite(
            c @ ResolvedCompositeType {
                kind: ResolvedCompositeTypeKind::Persistent { .. },
                ..
            },
        ) = &model
        {
            let model_type_id = building.get_id(c.name.as_str(), resolved_env).unwrap();
            let shallow_query = shallow_pk_query(model_type_id, c);
            let collection_query = shallow_collection_query(model_type_id, c);

            building
                .queries
                .add(&shallow_query.name.to_owned(), shallow_query);
            building
                .queries
                .add(&collection_query.name.to_owned(), collection_query);
        }
    }
}

pub fn build_expanded(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    for (model_type_id, model_type) in building.database_types.iter() {
        if let GqlTypeKind::Composite(GqlCompositeType { .. }) = &model_type.kind {
            {
                let operation_name = model_type.pk_query();
                let query = expanded_pk_query(model_type_id, model_type, building);
                let existing_id = building.queries.get_id(&operation_name).unwrap();
                building.queries[existing_id] = query;
            }
            {
                let operation_name = model_type.collection_query();
                let query =
                    expanded_collection_query(model_type_id, model_type, resolved_env, building);
                let existing_id = building.queries.get_id(&operation_name).unwrap();
                building.queries[existing_id] = query;
            }
        }
    }
}

fn shallow_pk_query(
    model_type_id: SerializableSlabIndex<GqlType>,
    typ: &ResolvedCompositeType,
) -> Query {
    let operation_name = typ.pk_query();
    Query {
        name: operation_name,
        kind: QueryKind::Database(Box::new(DatabaseQueryParameter {
            predicate_param: None,
            order_by_param: None,
            limit_param: None,
            offset_param: None,
        })),
        return_type: OperationReturnType {
            type_id: model_type_id,
            is_primitive: false,
            type_name: typ.name.clone(),
            type_modifier: GqlTypeModifier::NonNull,
            is_persistent: true,
        },
        interceptors: Interceptors::default(),
    }
}

fn expanded_pk_query(
    model_type_id: SerializableSlabIndex<GqlType>,
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> Query {
    let operation_name = model_type.pk_query();
    let existing_query = building.queries.get_by_key(&operation_name).unwrap();

    let pk_param = pk_predicate_param(model_type_id, model_type, building);

    Query {
        name: operation_name,
        kind: QueryKind::Database(Box::new(DatabaseQueryParameter {
            predicate_param: Some(pk_param),
            order_by_param: None,
            limit_param: None,
            offset_param: None,
        })),
        return_type: existing_query.return_type.clone(),
        interceptors: existing_query.interceptors.clone(),
    }
}

pub fn pk_predicate_param(
    model_type_id: SerializableSlabIndex<GqlType>,
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> PredicateParameter {
    let pk_field = model_type.pk_field().unwrap();

    PredicateParameter {
        name: pk_field.name.to_string(),
        type_name: pk_field.typ.type_name().to_string(),
        typ: PredicateParameterTypeWithModifier {
            type_id: building
                .predicate_types
                .get_id(pk_field.typ.type_name())
                .unwrap(),
            type_modifier: GqlTypeModifier::NonNull,
        },
        column_path_link: pk_field
            .relation
            .self_column()
            .map(|column_id| ColumnIdPathLink {
                self_column_id: column_id,
                linked_column_id: None,
            }),
        underlying_type_id: model_type_id,
    }
}

fn shallow_collection_query(
    model_type_id: SerializableSlabIndex<GqlType>,
    model: &ResolvedCompositeType,
) -> Query {
    let operation_name = model.collection_query();
    Query {
        name: operation_name,
        kind: QueryKind::Database(Box::new(DatabaseQueryParameter {
            predicate_param: None,
            order_by_param: None,
            limit_param: None,
            offset_param: None,
        })),
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: model.name.clone(),
            is_primitive: false,
            type_modifier: GqlTypeModifier::List,
            is_persistent: true,
        },
        interceptors: Interceptors::default(),
    }
}

fn expanded_collection_query(
    model_type_id: SerializableSlabIndex<GqlType>,
    model_type: &GqlType,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Query {
    let operation_name = model_type.collection_query();
    let existing_query = building.queries.get_by_key(&operation_name).unwrap();

    let predicate_param = collection_predicate_param(model_type_id, model_type, building);
    let order_by_param = order_by_type_builder::new_root_param(&model_type.name, false, building);
    let limit_param = limit_param(resolved_env, building);
    let offset_param = offset_param(resolved_env, building);

    Query {
        name: operation_name.clone(),
        kind: QueryKind::Database(Box::new(DatabaseQueryParameter {
            predicate_param: Some(predicate_param),
            order_by_param: Some(order_by_param),
            limit_param: Some(limit_param),
            offset_param: Some(offset_param),
        })),
        return_type: existing_query.return_type.clone(),
        interceptors: Interceptors::default(),
    }
}

pub fn limit_param(
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> LimitParameter {
    let param_type_name = "Int".to_string();

    LimitParameter {
        name: "limit".to_string(),
        typ: LimitParameterType {
            type_name: param_type_name.clone(),
            type_id: building.get_id(&param_type_name, resolved_env).unwrap(),
            type_modifier: GqlTypeModifier::Optional,
        },
    }
}

pub fn offset_param(
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> OffsetParameter {
    let param_type_name = "Int".to_string();

    OffsetParameter {
        name: "offset".to_string(),
        typ: OffsetParameterType {
            type_name: param_type_name.clone(),
            type_id: building.get_id(&param_type_name, resolved_env).unwrap(),
            type_modifier: GqlTypeModifier::Optional,
        },
    }
}

pub fn collection_predicate_param(
    model_type_id: SerializableSlabIndex<GqlType>,
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> PredicateParameter {
    let param_type_name = predicate_builder::get_parameter_type_name(&model_type.name);
    PredicateParameter {
        name: "where".to_string(),
        type_name: param_type_name.clone(),
        typ: PredicateParameterTypeWithModifier {
            type_id: building.predicate_types.get_id(&param_type_name).unwrap(),
            type_modifier: GqlTypeModifier::Optional,
        },
        column_path_link: None,
        underlying_type_id: model_type_id,
    }
}
