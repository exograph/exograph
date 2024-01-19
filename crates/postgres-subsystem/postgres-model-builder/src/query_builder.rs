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
    types::{BaseOperationReturnType, FieldType, Named, OperationReturnType},
};

use exo_sql::Database;
use postgres_model::{
    limit_offset::{LimitParameter, LimitParameterType, OffsetParameter, OffsetParameterType},
    order::{OrderByParameter, OrderByParameterType},
    predicate::{PredicateParameter, PredicateParameterType, PredicateParameterTypeWrapper},
    query::{
        AggregateQuery, AggregateQueryParameters, CollectionQuery, CollectionQueryParameters,
        PkQuery, PkQueryParameters, UniqueQuery, UniqueQueryParameters,
    },
    relation::PostgresRelation,
    types::{EntityType, PostgresField, PostgresPrimitiveType},
};

use crate::{
    aggregate_type_builder::aggregate_type_name, predicate_builder::get_unique_filter_type_name,
    resolved_builder::ResolvedCompositeType, shallow::Shallow, type_builder::ResolvedTypeEnv,
};

use super::{
    naming::ToPostgresQueryName, order_by_type_builder, predicate_builder,
    resolved_builder::ResolvedType, system_builder::SystemContextBuilding,
};

pub fn build_shallow(types: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    for (_, typ) in types.iter() {
        if let ResolvedType::Composite(c) = &typ {
            let entity_type_id = building.get_entity_type_id(c.name.as_str()).unwrap();
            let shallow_query = shallow_pk_query(entity_type_id, c);
            let collection_query = shallow_collection_query(entity_type_id, c);
            let aggregate_query = shallow_aggregate_query(entity_type_id, c);
            let unique_queries = shallow_unique_queries(entity_type_id, c);

            building
                .pk_queries
                .add(&shallow_query.name.to_owned(), shallow_query);
            building
                .collection_queries
                .add(&collection_query.name.to_owned(), collection_query);
            building
                .aggregate_queries
                .add(&aggregate_query.name.to_owned(), aggregate_query);
            for unique_query in unique_queries {
                building
                    .unique_queries
                    .add(&unique_query.name.to_owned(), unique_query);
            }
        }
    }
}

pub fn build_expanded(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    for (_, entity_type) in building.entity_types.iter() {
        expand_pk_query(
            entity_type,
            &building.predicate_types,
            &mut building.pk_queries,
            &building.database,
        );
        expand_collection_query(
            entity_type,
            &building.primitive_types,
            &building.predicate_types,
            &building.order_by_types,
            &mut building.collection_queries,
        );
        expand_aggregate_query(
            entity_type,
            &building.predicate_types,
            &mut building.aggregate_queries,
        );
        expand_unique_queries(
            entity_type,
            &building.predicate_types,
            &mut building.unique_queries,
            resolved_env,
            &building.database,
        );
    }
}

fn shallow_pk_query(
    entity_type_id: SerializableSlabIndex<EntityType>,
    typ: &ResolvedCompositeType,
) -> PkQuery {
    let operation_name = typ.pk_query();
    PkQuery {
        name: operation_name,
        parameters: PkQueryParameters {
            predicate_param: PredicateParameter::shallow(),
        },
        return_type: OperationReturnType::Optional(Box::new(OperationReturnType::Plain(
            BaseOperationReturnType {
                associated_type_id: entity_type_id,
                type_name: typ.name.clone(),
            },
        ))),
    }
}

fn expand_pk_query(
    entity_type: &EntityType,
    predicate_types: &MappedArena<PredicateParameterType>,
    pk_queries: &mut MappedArena<PkQuery>,
    database: &Database,
) {
    let operation_name = entity_type.pk_query();
    let existing_query = &mut pk_queries.get_by_key_mut(&operation_name).unwrap();
    existing_query.parameters.predicate_param =
        pk_predicate_param(entity_type, predicate_types, database);
}

pub fn pk_predicate_param(
    entity_type: &EntityType,
    predicate_types: &MappedArena<PredicateParameterType>,
    database: &Database,
) -> PredicateParameter {
    let pk_field = entity_type.pk_field().unwrap();
    implicit_equals_predicate_param(pk_field, predicate_types, database)
}

fn implicit_equals_predicate_param(
    field: &PostgresField<EntityType>,
    predicate_types: &MappedArena<PredicateParameterType>,
    database: &Database,
) -> PredicateParameter {
    let param_type_id = predicate_types.get_id(field.typ.name()).unwrap();
    let param_type = PredicateParameterTypeWrapper {
        name: field.typ.name().to_owned(),
        type_id: param_type_id,
    };

    PredicateParameter {
        name: field.name.to_string(),
        typ: FieldType::Plain(param_type),
        column_path_link: Some(field.relation.column_path_link(database)),
        access: None,
    }
}

fn shallow_collection_query(
    entity_type_id: SerializableSlabIndex<EntityType>,
    resolved_entity_type: &ResolvedCompositeType,
) -> CollectionQuery {
    let operation_name = resolved_entity_type.collection_query();
    CollectionQuery {
        name: operation_name,
        parameters: CollectionQueryParameters {
            predicate_param: PredicateParameter::shallow(),
            order_by_param: OrderByParameter::shallow(),
            limit_param: LimitParameter::shallow(),
            offset_param: OffsetParameter::shallow(),
        },
        return_type: OperationReturnType::List(Box::new(OperationReturnType::Plain(
            BaseOperationReturnType {
                associated_type_id: entity_type_id,
                type_name: resolved_entity_type.name.clone(),
            },
        ))),
    }
}

fn expand_collection_query(
    entity_type: &EntityType,
    primitive_types: &MappedArena<PostgresPrimitiveType>,
    predicate_types: &MappedArena<PredicateParameterType>,
    order_by_types: &MappedArena<OrderByParameterType>,
    collection_queries: &mut MappedArena<CollectionQuery>,
) {
    let operation_name = entity_type.collection_query();

    let predicate_param = collection_predicate_param(entity_type, predicate_types);
    let order_by_param =
        order_by_type_builder::new_root_param(&entity_type.name, false, order_by_types);
    let limit_param = limit_param(primitive_types);
    let offset_param = offset_param(primitive_types);

    let existing_query = &mut collection_queries.get_by_key_mut(&operation_name).unwrap();

    existing_query.parameters.predicate_param = predicate_param;
    existing_query.parameters.order_by_param = order_by_param;
    existing_query.parameters.limit_param = limit_param;
    existing_query.parameters.offset_param = offset_param;
}

fn shallow_aggregate_query(
    entity_type_id: SerializableSlabIndex<EntityType>,
    resolved_entity_type: &ResolvedCompositeType,
) -> AggregateQuery {
    AggregateQuery {
        name: resolved_entity_type.aggregate_query(),
        parameters: AggregateQueryParameters {
            predicate_param: PredicateParameter::shallow(),
        },
        return_type: OperationReturnType::Plain(BaseOperationReturnType {
            associated_type_id: entity_type_id,
            type_name: aggregate_type_name(&resolved_entity_type.name),
        }),
    }
}

fn expand_aggregate_query(
    entity_type: &EntityType,
    predicate_types: &MappedArena<PredicateParameterType>,
    aggregate_queries: &mut MappedArena<AggregateQuery>,
) {
    let operation_name = entity_type.aggregate_query();

    let predicate_param = collection_predicate_param(entity_type, predicate_types);

    let existing_query = &mut aggregate_queries.get_by_key_mut(&operation_name).unwrap();
    existing_query.parameters.predicate_param = predicate_param;
}

fn shallow_unique_queries(
    entity_type_id: SerializableSlabIndex<EntityType>,
    resolved_entity_type: &ResolvedCompositeType,
) -> Vec<UniqueQuery> {
    resolved_entity_type
        .unique_constraints()
        .keys()
        .map(|name| UniqueQuery {
            name: resolved_entity_type.unique_query(name),
            parameters: UniqueQueryParameters {
                predicate_params: vec![],
            },
            return_type: OperationReturnType::Optional(Box::new(OperationReturnType::Plain(
                BaseOperationReturnType {
                    associated_type_id: entity_type_id,
                    type_name: resolved_entity_type.name.clone(),
                },
            ))),
        })
        .collect()
}

pub fn expand_unique_queries(
    entity_type: &EntityType,
    predicate_types: &MappedArena<PredicateParameterType>,
    unique_queries: &mut MappedArena<UniqueQuery>,
    resolved_env: &ResolvedTypeEnv,
    database: &Database,
) {
    let resolved_type = resolved_env.get_by_key(entity_type.name.as_str()).unwrap();

    if let ResolvedType::Composite(resolved_composite_type) = resolved_type {
        for (name, fields) in resolved_composite_type.unique_constraints().iter() {
            let operation_name = entity_type.unique_query(name);

            let predicate_params = fields
                .iter()
                .map(|field| {
                    let entity_field = entity_type.field_by_name(&field.name).unwrap();

                    match entity_field.relation {
                        PostgresRelation::Pk { .. } | PostgresRelation::Scalar { .. } => {
                            implicit_equals_predicate_param(entity_field, predicate_types, database)
                        }
                        PostgresRelation::ManyToOne { .. } => {
                            let param_type_name = get_unique_filter_type_name(field.typ.name());
                            let param_type_id = predicate_types.get_id(&param_type_name).unwrap();
                            let param_type = PredicateParameterTypeWrapper {
                                name: param_type_name,
                                type_id: param_type_id,
                            };

                            PredicateParameter {
                                name: field.name.to_string(),
                                typ: FieldType::Plain(param_type),
                                column_path_link: Some(
                                    entity_field.relation.column_path_link(database),
                                ),
                                access: None,
                            }
                        }
                        PostgresRelation::OneToMany { .. } => {
                            panic!("OneToMany relations cannot be used in unique queries")
                        }
                    }
                })
                .collect();

            let existing_query = &mut unique_queries.get_by_key_mut(&operation_name).unwrap();
            existing_query.parameters.predicate_params = predicate_params;
        }
    }
}

pub fn limit_param(primitive_types: &MappedArena<PostgresPrimitiveType>) -> LimitParameter {
    let param_type_name = "Int".to_string();

    LimitParameter {
        name: "limit".to_string(),
        typ: FieldType::Optional(Box::new(FieldType::Plain(LimitParameterType {
            type_name: param_type_name.clone(),
            type_id: primitive_types.get_id(&param_type_name).unwrap(),
        }))),
    }
}

pub fn offset_param(primitive_types: &MappedArena<PostgresPrimitiveType>) -> OffsetParameter {
    let param_type_name = "Int".to_string();

    OffsetParameter {
        name: "offset".to_string(),
        typ: FieldType::Optional(Box::new(FieldType::Plain(OffsetParameterType {
            type_name: param_type_name.clone(),
            type_id: primitive_types.get_id(&param_type_name).unwrap(),
        }))),
    }
}

pub fn collection_predicate_param(
    entity_type: &EntityType,
    predicate_types: &MappedArena<PredicateParameterType>,
) -> PredicateParameter {
    let param_type_name = predicate_builder::get_filter_type_name(&entity_type.name);
    let param_type_id = predicate_types.get_id(&param_type_name).unwrap();

    let param_type = PredicateParameterTypeWrapper {
        name: param_type_name,
        type_id: param_type_id,
    };

    PredicateParameter {
        name: "where".to_string(),
        typ: FieldType::Optional(Box::new(FieldType::Plain(param_type))),
        column_path_link: None,
        access: None,
    }
}

impl Shallow for LimitParameter {
    fn shallow() -> Self {
        LimitParameter {
            name: String::default(),
            typ: FieldType::Plain(LimitParameterType::shallow()),
        }
    }
}

impl Shallow for LimitParameterType {
    fn shallow() -> Self {
        LimitParameterType {
            type_name: String::default(),
            type_id: SerializableSlabIndex::shallow(),
        }
    }
}

impl Shallow for OffsetParameter {
    fn shallow() -> Self {
        OffsetParameter {
            name: String::default(),
            typ: FieldType::Plain(OffsetParameterType::shallow()),
        }
    }
}

impl Shallow for OffsetParameterType {
    fn shallow() -> Self {
        OffsetParameterType {
            type_name: String::default(),
            type_id: SerializableSlabIndex::shallow(),
        }
    }
}
