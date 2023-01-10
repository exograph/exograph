use core_plugin_interface::core_model::mapped_arena::{MappedArena, SerializableSlabIndex};

use postgres_model::{
    column_path::ColumnIdPathLink,
    limit_offset::{LimitParameter, LimitParameterType, OffsetParameter, OffsetParameterType},
    operation::{
        AggregateQuery, AggregateQueryParameter, CollectionQuery, CollectionQueryParameter,
        OperationReturnType, PkQuery, PkQueryParameter,
    },
    order::OrderByParameter,
    predicate::{PredicateParameter, PredicateParameterTypeWithModifier},
    types::{PostgresCompositeType, PostgresTypeIndex, PostgresTypeModifier},
};

use crate::{aggregate_type_builder::aggregate_type_name, shallow::Shallow};

use super::{
    naming::ToPostgresQueryName,
    order_by_type_builder, predicate_builder,
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
};

pub fn build_shallow(types: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    for (_, typ) in types.iter() {
        if let ResolvedType::Composite(c) = &typ {
            let model_type_id = building.get_entity_type_id(c.name.as_str()).unwrap();
            let shallow_query = shallow_pk_query(model_type_id, c);
            let collection_query = shallow_collection_query(model_type_id, c);
            let aggregate_query = shallow_aggregate_query(model_type_id, c);

            building
                .pk_queries
                .add(&shallow_query.name.to_owned(), shallow_query);
            building
                .collection_queries
                .add(&collection_query.name.to_owned(), collection_query);
            building
                .aggregate_queries
                .add(&aggregate_query.name.to_owned(), aggregate_query);
        }
    }
}

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (model_type_id, model_type) in building.entity_types.iter() {
        {
            let query = expanded_pk_query(model_type_id, model_type, building);
            let existing_id = building.pk_queries.get_id(&query.name).unwrap();
            building.pk_queries[existing_id] = query;
        }
        {
            let query = expanded_collection_query(model_type_id, model_type, building);
            let existing_id = building.collection_queries.get_id(&query.name).unwrap();
            building.collection_queries[existing_id] = query;
        }
        {
            let query = expanded_aggregate_query(model_type_id, model_type, building);
            let existing_id = building.aggregate_queries.get_id(&query.name).unwrap();
            building.aggregate_queries[existing_id] = query;
        }
    }
}

fn shallow_pk_query(
    model_type_id: SerializableSlabIndex<PostgresCompositeType>,
    typ: &ResolvedCompositeType,
) -> PkQuery {
    let operation_name = typ.pk_query();
    PkQuery {
        name: operation_name,
        parameter: PkQueryParameter {
            predicate_param: PredicateParameter::shallow(),
        },
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: typ.name.clone(),
            type_modifier: PostgresTypeModifier::NonNull,
        },
    }
}

fn expanded_pk_query(
    model_type_id: SerializableSlabIndex<PostgresCompositeType>,
    model_type: &PostgresCompositeType,
    building: &SystemContextBuilding,
) -> PkQuery {
    let operation_name = model_type.pk_query();
    let existing_query = building.pk_queries.get_by_key(&operation_name).unwrap();

    let pk_param = pk_predicate_param(model_type_id, model_type, building);

    PkQuery {
        name: operation_name,
        parameter: PkQueryParameter {
            predicate_param: pk_param,
        },
        return_type: existing_query.return_type.clone(),
    }
}

pub fn pk_predicate_param(
    model_type_id: SerializableSlabIndex<PostgresCompositeType>,
    model_type: &PostgresCompositeType,
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
            type_modifier: PostgresTypeModifier::NonNull,
        },
        column_path_link: pk_field
            .relation
            .self_column()
            .map(|column_id| ColumnIdPathLink {
                self_column_id: column_id,
                linked_column_id: None,
            }),
        underlying_type_id: PostgresTypeIndex::Composite(model_type_id),
    }
}

fn shallow_collection_query(
    model_type_id: SerializableSlabIndex<PostgresCompositeType>,
    model: &ResolvedCompositeType,
) -> CollectionQuery {
    let operation_name = model.collection_query();
    CollectionQuery {
        name: operation_name,
        parameter: CollectionQueryParameter {
            predicate_param: PredicateParameter::shallow(),
            order_by_param: OrderByParameter::shallow(),
            limit_param: LimitParameter::shallow(),
            offset_param: OffsetParameter::shallow(),
        },
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: model.name.clone(),
            type_modifier: PostgresTypeModifier::List,
        },
    }
}

fn expanded_collection_query(
    model_type_id: SerializableSlabIndex<PostgresCompositeType>,
    model_type: &PostgresCompositeType,
    building: &SystemContextBuilding,
) -> CollectionQuery {
    let operation_name = model_type.collection_query();
    let existing_query = building
        .collection_queries
        .get_by_key(&operation_name)
        .unwrap();

    let predicate_param = collection_predicate_param(model_type_id, model_type, building);
    let order_by_param = order_by_type_builder::new_root_param(&model_type.name, false, building);
    let limit_param = limit_param(building);
    let offset_param = offset_param(building);

    CollectionQuery {
        name: operation_name.clone(),
        parameter: CollectionQueryParameter {
            predicate_param,
            order_by_param,
            limit_param,
            offset_param,
        },
        return_type: existing_query.return_type.clone(),
    }
}

fn shallow_aggregate_query(
    model_type_id: SerializableSlabIndex<PostgresCompositeType>,
    typ: &ResolvedCompositeType,
) -> AggregateQuery {
    AggregateQuery {
        name: typ.aggregate_query(),
        parameter: AggregateQueryParameter {
            predicate_param: PredicateParameter::shallow(),
        },
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: aggregate_type_name(&typ.name),
            type_modifier: PostgresTypeModifier::NonNull,
        },
    }
}

fn expanded_aggregate_query(
    model_type_id: SerializableSlabIndex<PostgresCompositeType>,
    model_type: &PostgresCompositeType,
    building: &SystemContextBuilding,
) -> AggregateQuery {
    let operation_name = model_type.aggregate_query();
    let existing_query = building
        .aggregate_queries
        .get_by_key(&operation_name)
        .unwrap();

    let predicate_param = collection_predicate_param(model_type_id, model_type, building);

    AggregateQuery {
        name: operation_name.clone(),
        parameter: AggregateQueryParameter { predicate_param },
        return_type: existing_query.return_type.clone(),
    }
}

pub fn limit_param(building: &SystemContextBuilding) -> LimitParameter {
    let param_type_name = "Int".to_string();

    LimitParameter {
        name: "limit".to_string(),
        typ: LimitParameterType {
            type_name: param_type_name.clone(),
            type_id: building.get_primitive_type_id(&param_type_name).unwrap(),
            type_modifier: PostgresTypeModifier::Optional,
        },
    }
}

pub fn offset_param(building: &SystemContextBuilding) -> OffsetParameter {
    let param_type_name = "Int".to_string();

    OffsetParameter {
        name: "offset".to_string(),
        typ: OffsetParameterType {
            type_name: param_type_name.clone(),
            type_id: building.get_primitive_type_id(&param_type_name).unwrap(),
            type_modifier: PostgresTypeModifier::Optional,
        },
    }
}

pub fn collection_predicate_param(
    model_type_id: SerializableSlabIndex<PostgresCompositeType>,
    model_type: &PostgresCompositeType,
    building: &SystemContextBuilding,
) -> PredicateParameter {
    let param_type_name = predicate_builder::get_parameter_type_name(&model_type.name);
    PredicateParameter {
        name: "where".to_string(),
        type_name: param_type_name.clone(),
        typ: PredicateParameterTypeWithModifier {
            type_id: building.predicate_types.get_id(&param_type_name).unwrap(),
            type_modifier: PostgresTypeModifier::Optional,
        },
        column_path_link: None,
        underlying_type_id: PostgresTypeIndex::Composite(model_type_id),
    }
}

impl Shallow for LimitParameter {
    fn shallow() -> Self {
        LimitParameter {
            name: String::default(),
            typ: LimitParameterType::shallow(),
        }
    }
}

impl Shallow for LimitParameterType {
    fn shallow() -> Self {
        LimitParameterType {
            type_name: String::default(),
            type_id: SerializableSlabIndex::shallow(),
            type_modifier: PostgresTypeModifier::Optional,
        }
    }
}

impl Shallow for OffsetParameter {
    fn shallow() -> Self {
        OffsetParameter {
            name: String::default(),
            typ: OffsetParameterType::shallow(),
        }
    }
}

impl Shallow for OffsetParameterType {
    fn shallow() -> Self {
        OffsetParameterType {
            type_name: String::default(),
            type_id: SerializableSlabIndex::shallow(),
            type_modifier: PostgresTypeModifier::Optional,
        }
    }
}
