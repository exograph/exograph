// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

// TODO: This is duplicated from postgres-builder and cli
#[cfg(test)]
use core_plugin_interface::{
    error::ModelSerializationError, serializable_system::SerializableSystem,
    system_serializer::SystemSerializer,
};
#[cfg(test)]
use postgres_graphql_model::subsystem::PostgresSubsystem;

#[cfg(test)]
pub(crate) async fn create_postgres_system_from_str(
    model_str: &str,
    file_name: String,
) -> Result<PostgresSubsystem, ModelSerializationError> {
    let system = builder::build_system_from_str(
        model_str,
        file_name,
        vec![Box::new(
            postgres_builder::PostgresSubsystemBuilder::default(),
        )],
    )
    .await
    .unwrap();

    deserialize_postgres_subsystem(system)
}

#[cfg(test)]
fn deserialize_postgres_subsystem(
    system: SerializableSystem,
) -> Result<PostgresSubsystem, ModelSerializationError> {
    system
        .subsystems
        .into_iter()
        .find_map(|subsystem| {
            if subsystem.id == "postgres" {
                subsystem
                    .graphql
                    .map(|graphql| PostgresSubsystem::deserialize(graphql.0))
            } else {
                None
            }
        })
        // If there is no database subsystem in the serialized system, create an empty one
        .unwrap_or_else(|| Ok(PostgresSubsystem::default()))
}
