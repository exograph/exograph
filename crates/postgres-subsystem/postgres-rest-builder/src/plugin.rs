// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use core_plugin_interface::core_model_builder::plugin::RestSubsystemBuild;

use core_plugin_interface::serializable_system::SerializableRestBytes;
use core_plugin_interface::{
    core_model_builder::error::ModelBuildingError, system_serializer::SystemSerializer,
};

use postgres_core_builder::resolved_type::ResolvedType;
use postgres_core_builder::resolved_type::ResolvedTypeEnv;
use postgres_rest_model::method::Method;
use postgres_rest_model::operation::{PostgresOperation, PostgresOperationKind};
use postgres_rest_model::subsystem::PostgresRestSubsystem;

pub struct PostgresRestSubsystemBuilder {}

impl PostgresRestSubsystemBuilder {
    pub async fn build<'a>(
        &self,
        resolved_env: &ResolvedTypeEnv<'a>,
        core_subsystem_building: Arc<postgres_core_builder::SystemContextBuilding>,
    ) -> Result<Option<RestSubsystemBuild>, ModelBuildingError> {
        let mut operations = vec![];

        for typ in resolved_env.resolved_types.iter() {
            if let ResolvedType::Composite(composite) = typ.1 {
                let table_id = core_subsystem_building
                    .database
                    .get_table_id(&composite.table_name)
                    .ok_or(ModelBuildingError::Generic(format!(
                        "Table not found: {}",
                        composite.table_name.fully_qualified_name()
                    )))?;

                operations.push((
                    Method::Get,
                    composite.plural_name.to_lowercase(),
                    PostgresOperation {
                        kind: PostgresOperationKind::Query,
                        table_id,
                    },
                ));
            }
        }

        if operations.is_empty() {
            return Ok(None);
        }

        let subsystem = PostgresRestSubsystem {
            operations,
            core_subsystem: Default::default(),
        };

        let serialized_subsystem = subsystem
            .serialize()
            .map_err(ModelBuildingError::Serialize)?;

        Ok(Some(RestSubsystemBuild {
            serialized_subsystem: SerializableRestBytes(serialized_subsystem),
        }))
    }
}
