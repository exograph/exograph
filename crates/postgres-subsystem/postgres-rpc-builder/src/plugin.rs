// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use core_model::mapped_arena::MappedArena;
use core_model::types::{BaseOperationReturnType, FieldType, Named, OperationReturnType};
use core_model_builder::plugin::RpcSubsystemBuild;
use core_plugin_shared::{
    serializable_system::SerializableRpcBytes, system_serializer::SystemSerializer,
};

use core_model_builder::error::ModelBuildingError;

use postgres_core_builder::order_by_builder::new_root_param;
use postgres_core_builder::predicate_builder::get_filter_type_name;
use postgres_core_builder::resolved_type::ResolvedType;
use postgres_core_builder::resolved_type::ResolvedTypeEnv;
use postgres_core_model::predicate::{PredicateParameter, PredicateParameterTypeWrapper};
use postgres_core_model::relation::PostgresRelation;
use postgres_core_model::types::EntityRepresentation;
use postgres_rpc_model::operation::{
    CollectionQuery, CollectionQueryParameters, PkQuery, PkQueryParameters,
};
use postgres_rpc_model::subsystem::PostgresRpcSubsystem;

pub struct PostgresRpcSubsystemBuilder {}

impl PostgresRpcSubsystemBuilder {
    pub async fn build(
        &self,
        resolved_env: &ResolvedTypeEnv<'_>,
        core_subsystem_building: Arc<postgres_core_builder::SystemContextBuilding>,
    ) -> Result<Option<RpcSubsystemBuild>, ModelBuildingError> {
        let mut collection_queries = MappedArena::default();
        let mut pk_queries = MappedArena::default();

        for typ in resolved_env.resolved_types.iter() {
            if let ResolvedType::Composite(composite) = typ.1 {
                if composite.representation == EntityRepresentation::Json {
                    continue;
                }

                let entity_type_id = core_subsystem_building
                    .entity_types
                    .get_id(&composite.name)
                    .ok_or(ModelBuildingError::Generic(format!(
                        "Entity type not found: {}",
                        composite.name
                    )))?;

                let entity_type = &core_subsystem_building.entity_types[entity_type_id];

                // Build collection query (such as get_todos) - returns multiple items
                let collection_method = format!("get_{}", composite.plural_name.to_lowercase());

                let predicate_param = {
                    let param_type_name = get_filter_type_name(&composite.name);
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

                    PredicateParameter {
                        name: "where".to_string(),
                        typ: FieldType::Optional(Box::new(FieldType::Plain(param_type))),
                        column_path_link: None,
                        access: None,
                        vector_distance_function: None,
                    }
                };

                let order_by_param = new_root_param(
                    &composite.name,
                    false,
                    &core_subsystem_building.order_by_types,
                );

                // Return type: List of the entity type
                let return_type: OperationReturnType<_> =
                    FieldType::List(Box::new(FieldType::Plain(BaseOperationReturnType {
                        associated_type_id: entity_type_id,
                        type_name: composite.name.clone(),
                    })));

                let collection_query = CollectionQuery {
                    name: collection_method.clone(),
                    parameters: CollectionQueryParameters {
                        predicate_param,
                        order_by_param,
                    },
                    return_type,
                };

                collection_queries.add(&collection_method, collection_query);

                // Build pk query (get_todo) - returns single item by primary key
                // Only create pk query if all pk fields are scalar (simple pk, not composite with relations)
                let pk_fields = entity_type.pk_fields();
                let all_scalar = pk_fields
                    .iter()
                    .all(|field| matches!(field.relation, PostgresRelation::Scalar { .. }));

                if !pk_fields.is_empty() && all_scalar {
                    let pk_method = format!("get_{}", composite.name.to_lowercase());

                    let pk_params: Vec<PredicateParameter> = pk_fields
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
                        .collect();

                    // Return type: Optional of the entity type
                    let return_type: OperationReturnType<_> =
                        FieldType::Optional(Box::new(FieldType::Plain(BaseOperationReturnType {
                            associated_type_id: entity_type_id,
                            type_name: composite.name.clone(),
                        })));

                    let pk_query = PkQuery {
                        name: pk_method.clone(),
                        parameters: PkQueryParameters {
                            predicate_params: pk_params,
                        },
                        return_type,
                    };

                    pk_queries.add(&pk_method, pk_query);
                }
            }
        }

        if collection_queries.is_empty() && pk_queries.is_empty() {
            return Ok(None);
        }

        let subsystem = PostgresRpcSubsystem {
            pk_queries,
            collection_queries,
            core_subsystem: Default::default(),
        };

        let serialized_subsystem = subsystem
            .serialize()
            .map_err(ModelBuildingError::Serialize)?;

        Ok(Some(RpcSubsystemBuild {
            serialized_subsystem: SerializableRpcBytes(serialized_subsystem),
        }))
    }
}
