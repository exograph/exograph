// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use core_model::types::FieldType;
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
use postgres_core_model::types::EntityRepresentation;
use postgres_rpc_model::operation::{PostgresOperation, PostgresOperationKind};
use postgres_rpc_model::subsystem::PostgresRpcSubsystem;

pub struct PostgresRpcSubsystemBuilder {}

impl PostgresRpcSubsystemBuilder {
    pub async fn build(
        &self,
        resolved_env: &ResolvedTypeEnv<'_>,
        core_subsystem_building: Arc<postgres_core_builder::SystemContextBuilding>,
    ) -> Result<Option<RpcSubsystemBuild>, ModelBuildingError> {
        let mut operations = vec![];

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

                let rpc_method = format!("get_{}", composite.plural_name.to_lowercase());

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

                operations.push((
                    rpc_method,
                    PostgresOperation {
                        kind: PostgresOperationKind::Query,
                        entity_type_id,
                        predicate_param,
                        order_by_param,
                    },
                ));
            }
        }

        if operations.is_empty() {
            return Ok(None);
        }

        let subsystem = PostgresRpcSubsystem {
            operations,
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
