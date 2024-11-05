// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use core_plugin_interface::core_model_builder::plugin::RestSubsystemBuild;
use core_plugin_interface::interface::RestSubsystemBuilder;

use core_plugin_interface::serializable_system::SerializableRestBytes;
use core_plugin_interface::{
    core_model_builder::{
        builder::system_builder::BaseModelSystem, error::ModelBuildingError,
        typechecker::typ::TypecheckedSystem,
    },
    system_serializer::SystemSerializer,
};

use core_rest_model::path::PathTemplate;
use postgres_core_builder::resolved_builder;
use postgres_core_builder::resolved_type::ResolvedType;
use postgres_rest_model::operation::{Method, PostgresOperation};
use postgres_rest_model::subsystem::PostgresRestSubsystem;

pub struct PostgresRestSubsystemBuilder {}

#[async_trait]
impl RestSubsystemBuilder for PostgresRestSubsystemBuilder {
    fn id(&self) -> &'static str {
        "postgres"
    }

    async fn build(
        &self,
        typechecked_system: &TypecheckedSystem,
        _base_system: &BaseModelSystem,
    ) -> Result<Option<RestSubsystemBuild>, ModelBuildingError> {
        let resolved_types = resolved_builder::build(typechecked_system)?;

        let mut operations = vec![];

        for typ in resolved_types.iter() {
            if let ResolvedType::Composite(composite) = typ.1 {
                operations.push(PostgresOperation {
                    method: Method::Get,
                    path_template: PathTemplate::simple(&composite.plural_name),
                });
            }
        }

        if operations.is_empty() {
            return Ok(None);
        }

        let subsystem = PostgresRestSubsystem { operations };

        let serialized_subsystem = subsystem
            .serialize()
            .map_err(ModelBuildingError::Serialize)?;

        Ok(Some(RestSubsystemBuild {
            id: self.id().to_string(),
            serialized_subsystem: SerializableRestBytes(serialized_subsystem),
        }))
    }
}
