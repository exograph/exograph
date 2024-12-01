// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#[cfg(test)]
use core_plugin_interface::{
    error::ModelSerializationError, serializable_system::SerializableSystem,
    system_serializer::SystemSerializer,
};
#[cfg(test)]
use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;

#[cfg(test)]
pub(crate) async fn create_postgres_system_from_str(
    model_str: &str,
    file_name: String,
) -> Result<PostgresGraphQLSubsystem, ModelSerializationError> {
    let system = builder::build_system_from_str(
        model_str,
        file_name,
        vec![Box::new(crate::PostgresSubsystemBuilder::default())],
    )
    .await
    .unwrap();

    deserialize_postgres_subsystem(system)
}

#[cfg(test)]
fn deserialize_postgres_subsystem(
    system: SerializableSystem,
) -> Result<PostgresGraphQLSubsystem, ModelSerializationError> {
    use std::sync::Arc;

    use postgres_core_model::subsystem::PostgresCoreSubsystem;

    let postgres_subsystem = system
        .subsystems
        .into_iter()
        .find(|subsystem| subsystem.id == "postgres");

    match postgres_subsystem {
        Some(subsystem) => {
            let mut postgres_subsystem =
                PostgresGraphQLSubsystem::deserialize(subsystem.graphql.unwrap().0)?;
            let postgres_core_subsystem = PostgresCoreSubsystem::deserialize(subsystem.core.0)?;
            postgres_subsystem.database = Arc::new(postgres_core_subsystem.database);
            Ok(postgres_subsystem)
        }
        None => Ok(PostgresGraphQLSubsystem::default()),
    }
}
