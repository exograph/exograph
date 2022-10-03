use std::path::Path;

use payas_core_model::{
    serializable_system::SerializableSystem, system_serializer::SystemSerializer,
};
use payas_database_model::model::ModelDatabaseSystem;
use payas_parser::error::ParserError;

pub(crate) fn create_database_system(
    model_file: impl AsRef<Path>,
) -> Result<ModelDatabaseSystem, ParserError> {
    let serialized_system = payas_parser::build_system(&model_file)?;
    let system = SerializableSystem::deserialize(serialized_system)?;

    deserialize_database_subsystem(system)
}

#[cfg(test)]
pub(crate) fn create_database_system_from_str(
    model_str: &str,
    file_name: String,
) -> Result<ModelDatabaseSystem, ParserError> {
    let serialized_system = payas_parser::build_system_from_str(model_str, file_name)?;
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
        .ok_or_else(|| ParserError::Generic("No database subsystem found in model".into()))?
        .map_err(|e| {
            ParserError::Generic(format!(
                "Error while deserializing database subsystem: {}",
                e
            ))
        })
}
