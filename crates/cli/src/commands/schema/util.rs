use std::path::Path;

use builder::error::ParserError;
use core_plugin_shared::{
    serializable_system::SerializableSystem, system_serializer::SystemSerializer,
};
use postgres_model::model::ModelPostgresSystem;

pub(crate) fn create_postgres_system(
    model_file: impl AsRef<Path>,
) -> Result<ModelPostgresSystem, ParserError> {
    let serialized_system = builder::build_system(&model_file)?;
    let system = SerializableSystem::deserialize(serialized_system)?;

    deserialize_postgres_subsystem(system)
}

#[cfg(test)]
pub(crate) fn create_postgres_system_from_str(
    model_str: &str,
    file_name: String,
) -> Result<ModelPostgresSystem, ParserError> {
    let serialized_system = builder::build_system_from_str(model_str, file_name)?;
    let system = SerializableSystem::deserialize(serialized_system)?;

    deserialize_postgres_subsystem(system)
}

fn deserialize_postgres_subsystem(
    system: SerializableSystem,
) -> Result<ModelPostgresSystem, ParserError> {
    system
        .subsystems
        .into_iter()
        .find_map(|subsystem| {
            if subsystem.id == "postgres" {
                Some(ModelPostgresSystem::deserialize(
                    subsystem.serialized_subsystem,
                ))
            } else {
                None
            }
        })
        // If there is no database subsystem in the serialized system, create an empty one
        .unwrap_or_else(|| Ok(ModelPostgresSystem::default()))
        .map_err(|e| {
            ParserError::Generic(format!("Error while deserializing database subsystem: {e}"))
        })
}
