use std::path::Path;

use builder::error::ParserError;
use core_plugin::{serializable_system::SerializableSystem, system_serializer::SystemSerializer};
use database_model::model::ModelDatabaseSystem;

pub(crate) fn create_database_system(
    model_file: impl AsRef<Path>,
) -> Result<ModelDatabaseSystem, ParserError> {
    let serialized_system = builder::build_system(&model_file)?;
    let system = SerializableSystem::deserialize(serialized_system)?;

    deserialize_database_subsystem(system)
}

#[cfg(test)]
pub(crate) fn create_database_system_from_str(
    model_str: &str,
    file_name: String,
) -> Result<ModelDatabaseSystem, ParserError> {
    let serialized_system = builder::build_system_from_str(model_str, file_name)?;
    let system = SerializableSystem::deserialize(serialized_system)?;

    deserialize_database_subsystem(system)
}

fn deserialize_database_subsystem(
    system: SerializableSystem,
) -> Result<ModelDatabaseSystem, ParserError> {
    system
        .subsystems
        .into_iter()
        .find_map(|subsystem| {
            if subsystem.id == "database" {
                Some(ModelDatabaseSystem::deserialize(
                    subsystem.serialized_subsystem,
                ))
            } else {
                None
            }
        })
        // If there is no database subsystem in the serialized system, create an empty one
        .unwrap_or_else(|| Ok(ModelDatabaseSystem::default()))
        .map_err(|e| {
            ParserError::Generic(format!(
                "Error while deserializing database subsystem: {}",
                e
            ))
        })
}
