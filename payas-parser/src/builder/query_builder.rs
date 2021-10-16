use payas_model::model::limit_offset::OffsetParameter;
use payas_model::model::mapped_arena::SerializableSlabIndex;
use payas_model::model::naming::ToGqlQueryName;
use payas_model::model::operation::{DatabaseQueryParameter, QueryKind};
use payas_model::model::{
    limit_offset::LimitParameter,
    mapped_arena::MappedArena,
    operation::{OperationReturnType, Query},
    predicate::PredicateParameter,
    GqlType, GqlTypeKind, GqlTypeModifier,
};
use payas_model::model::{GqlCompositeType, GqlCompositeTypeKind};

use super::resolved_builder::ResolvedCompositeTypeKind;
use super::{
    order_by_type_builder, predicate_builder,
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
};

pub fn build_shallow(models: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    for (_, model) in models.iter() {
        if let ResolvedType::Composite(
            c
            @
            ResolvedCompositeType {
                kind: ResolvedCompositeTypeKind::Persistent { .. },
                ..
            },
        ) = &model
        {
            let model_type_id = building.types.get_id(c.name.as_str()).unwrap();
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

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (_, model_type) in building.types.iter() {
        if let GqlTypeKind::Composite(GqlCompositeType {
            kind: GqlCompositeTypeKind::Persistent { .. },
            ..
        }) = &model_type.kind
        {
            {
                let operation_name = model_type.pk_query();
                let query = expanded_pk_query(model_type, building);
                let existing_id = building.queries.get_id(&operation_name).unwrap();
                building.queries[existing_id] = query;
            }
            {
                let operation_name = model_type.collection_query();
                let query = expanded_collection_query(model_type, building);
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
        kind: QueryKind::Database(DatabaseQueryParameter {
            predicate_param: None,
            order_by_param: None,
            limit_param: None,
            offset_param: None,
        }),
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: typ.name.clone(),
            type_modifier: GqlTypeModifier::NonNull,
        },
    }
}

fn expanded_pk_query(model_type: &GqlType, building: &SystemContextBuilding) -> Query {
    let operation_name = model_type.pk_query();
    let existing_query = building.queries.get_by_key(&operation_name).unwrap();

    let pk_param = pk_predicate_param(model_type, building);

    Query {
        name: operation_name,
        kind: QueryKind::Database(DatabaseQueryParameter {
            predicate_param: Some(pk_param),
            order_by_param: None,
            limit_param: None,
            offset_param: None,
        }),
        return_type: existing_query.return_type.clone(),
    }
}

pub fn pk_predicate_param(
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> PredicateParameter {
    let pk_field = model_type.pk_field().unwrap();

    PredicateParameter {
        name: pk_field.name.to_string(),
        type_name: pk_field.typ.type_name().to_string(),
        type_id: building
            .predicate_types
            .get_id(pk_field.typ.type_name())
            .unwrap(),
        type_modifier: GqlTypeModifier::NonNull,
        column_id: pk_field.relation.self_column(),
    }
}

fn shallow_collection_query(
    model_type_id: SerializableSlabIndex<GqlType>,
    model: &ResolvedCompositeType,
) -> Query {
    let operation_name = model.collection_query();
    Query {
        name: operation_name,
        kind: QueryKind::Database(DatabaseQueryParameter {
            predicate_param: None,
            order_by_param: None,
            limit_param: None,
            offset_param: None,
        }),
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: model.name.clone(),
            type_modifier: GqlTypeModifier::List,
        },
    }
}

fn expanded_collection_query(model_type: &GqlType, building: &SystemContextBuilding) -> Query {
    let operation_name = model_type.collection_query();
    let existing_query = building.queries.get_by_key(&operation_name).unwrap();

    let predicate_param = collection_predicate_param(model_type, building);
    let order_by_param = order_by_type_builder::new_root_param(&model_type.name, false, building);
    let limit_param = limit_param(building);
    let offset_param = offset_param(building);

    Query {
        name: operation_name.clone(),
        kind: QueryKind::Database(DatabaseQueryParameter {
            predicate_param: Some(predicate_param),
            order_by_param: Some(order_by_param),
            limit_param: Some(limit_param),
            offset_param: Some(offset_param),
        }),
        return_type: existing_query.return_type.clone(),
    }
}

pub fn limit_param(building: &SystemContextBuilding) -> LimitParameter {
    let param_type_name = "Int".to_string();

    LimitParameter {
        name: "limit".to_string(),
        type_name: param_type_name.clone(),
        type_id: building.types.get_id(&param_type_name).unwrap(),
        type_modifier: GqlTypeModifier::Optional,
    }
}

pub fn offset_param(building: &SystemContextBuilding) -> OffsetParameter {
    let param_type_name = "Int".to_string();

    OffsetParameter {
        name: "offset".to_string(),
        type_name: param_type_name.clone(),
        type_id: building.types.get_id(&param_type_name).unwrap(),
        type_modifier: GqlTypeModifier::Optional,
    }
}

pub fn collection_predicate_param(
    model_type: &GqlType,
    building: &SystemContextBuilding,
) -> PredicateParameter {
    let param_type_name = predicate_builder::get_parameter_type_name(&model_type.name);
    PredicateParameter {
        name: "where".to_string(),
        type_name: param_type_name.clone(),
        type_id: building.predicate_types.get_id(&param_type_name).unwrap(),
        type_modifier: GqlTypeModifier::Optional,
        column_id: None,
    }
}
