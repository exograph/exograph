// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::mapped_arena::SerializableSlabIndex;
use core_model::types::{BaseOperationReturnType, FieldType, Named, OperationReturnType};
use core_model_builder::error::ModelBuildingError;

use postgres_core_builder::predicate_builder::{get_filter_type_name, get_unique_filter_type_name};
use postgres_core_builder::resolved_type::ResolvedField;
use postgres_core_model::predicate::{PredicateParameter, PredicateParameterTypeWrapper};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_model::types::{EntityType, PostgresField};

/// Build predicate parameters for PK fields.
/// Returns `None` if PK fields are empty or contain non-scalar relations.
pub fn build_pk_predicate_params(
    pk_fields: &[&PostgresField<EntityType>],
    core_subsystem_building: &postgres_core_builder::SystemContextBuilding,
) -> Option<Vec<PredicateParameter>> {
    let all_scalar = pk_fields
        .iter()
        .all(|field| matches!(field.relation, PostgresRelation::Scalar { .. }));

    if pk_fields.is_empty() || !all_scalar {
        return None;
    }

    Some(
        pk_fields
            .iter()
            .map(|field| {
                let predicate_type_name = field.typ.name().to_owned();
                let param_type_id = core_subsystem_building
                    .predicate_types
                    .get_id(&predicate_type_name)
                    .unwrap();
                let param_type = PredicateParameterTypeWrapper {
                    name: predicate_type_name,
                    type_id: param_type_id,
                };

                PredicateParameter {
                    name: field.name.to_string(),
                    typ: FieldType::Plain(param_type),
                    column_path_link: Some(
                        field
                            .relation
                            .column_path_link(&core_subsystem_building.database),
                    ),
                    access: None,
                    vector_distance_function: None,
                }
            })
            .collect(),
    )
}

/// Build predicate parameters for unique constraint fields.
pub fn build_unique_predicate_params(
    constraint_fields: &[&ResolvedField],
    entity_type: &EntityType,
    core_subsystem_building: &postgres_core_builder::SystemContextBuilding,
) -> Result<Vec<PredicateParameter>, ModelBuildingError> {
    constraint_fields
        .iter()
        .map(|field| {
            let entity_field = entity_type.field_by_name(&field.name).unwrap();

            let param_type_name = match &entity_field.relation {
                PostgresRelation::Scalar { .. } => entity_field.typ.name().to_owned(),
                PostgresRelation::ManyToOne { .. } => {
                    get_unique_filter_type_name(entity_field.typ.name())
                }
                _ => {
                    return Err(ModelBuildingError::Generic(format!(
                        "Unsupported relation type in unique constraint: {:?}",
                        entity_field.relation
                    )));
                }
            };

            let param_type_id = core_subsystem_building
                .predicate_types
                .get_id(&param_type_name)
                .unwrap();
            let param_type = PredicateParameterTypeWrapper {
                name: param_type_name,
                type_id: param_type_id,
            };

            Ok(PredicateParameter {
                name: entity_field.name.to_string(),
                typ: FieldType::Plain(param_type),
                column_path_link: Some(
                    entity_field
                        .relation
                        .column_path_link(&core_subsystem_building.database),
                ),
                access: None,
                vector_distance_function: None,
            })
        })
        .collect()
}

/// Build a filter predicate parameter (for `where` clauses in collection operations).
pub fn build_filter_predicate_param(
    entity_name: &str,
    core_subsystem_building: &postgres_core_builder::SystemContextBuilding,
) -> Result<PredicateParameter, ModelBuildingError> {
    let param_type_name = get_filter_type_name(entity_name);
    let param_type_id = core_subsystem_building
        .predicate_types
        .get_id(&param_type_name)
        .ok_or(ModelBuildingError::Generic(format!(
            "Predicate type not found: {}",
            param_type_name
        )))?;

    let param_type = PredicateParameterTypeWrapper {
        name: param_type_name,
        type_id: param_type_id,
    };

    Ok(PredicateParameter {
        name: postgres_core_model::predicate::PREDICATE_PARAM_NAME.to_string(),
        typ: FieldType::Optional(Box::new(FieldType::Plain(param_type))),
        column_path_link: None,
        access: None,
        vector_distance_function: None,
    })
}

/// Build a `List<Entity>` return type.
pub fn list_return_type(
    entity_type_id: SerializableSlabIndex<EntityType>,
    type_name: &str,
) -> OperationReturnType<EntityType> {
    FieldType::List(Box::new(FieldType::Plain(BaseOperationReturnType {
        associated_type_id: entity_type_id,
        type_name: type_name.to_string(),
    })))
}

/// Build an `Optional<Entity>` return type.
pub fn optional_return_type(
    entity_type_id: SerializableSlabIndex<EntityType>,
    type_name: &str,
) -> OperationReturnType<EntityType> {
    FieldType::Optional(Box::new(FieldType::Plain(BaseOperationReturnType {
        associated_type_id: entity_type_id,
        type_name: type_name.to_string(),
    })))
}
