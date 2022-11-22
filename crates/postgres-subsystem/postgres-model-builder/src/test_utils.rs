#[cfg(test)]
use core_plugin_interface::{
    error::ModelSerializationError, serializable_system::SerializableSystem,
    system_serializer::SystemSerializer,
};
#[cfg(test)]
use postgres_model::model::ModelPostgresSystem;

#[cfg(test)]
pub(crate) fn create_postgres_system_from_str(
    model_str: &str,
    file_name: String,
) -> Result<ModelPostgresSystem, ModelSerializationError> {
    let serialized_system = builder::build_system_from_str(model_str, file_name).unwrap();
    let system = SerializableSystem::deserialize(serialized_system)?;

    deserialize_postgres_subsystem(system)
}

#[cfg(test)]
fn deserialize_postgres_subsystem(
    system: SerializableSystem,
) -> Result<ModelPostgresSystem, ModelSerializationError> {
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
}
