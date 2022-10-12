// TODO: This is duplicated from payas-database-builder and payas-cli
#[cfg(test)]
use payas_core_plugin::{
    error::ModelSerializationError, serializable_system::SerializableSystem,
    system_serializer::SystemSerializer,
};
#[cfg(test)]
use payas_database_model::model::ModelDatabaseSystem;

#[cfg(test)]
pub(crate) fn create_database_system_from_str(
    model_str: &str,
    file_name: String,
) -> Result<ModelDatabaseSystem, ModelSerializationError> {
    let serialized_system = payas_builder::build_system_from_str(model_str, file_name).unwrap();
    let system = SerializableSystem::deserialize(serialized_system)?;

    deserialize_database_subsystem(system)
}

#[cfg(test)]
fn deserialize_database_subsystem(
    system: SerializableSystem,
) -> Result<ModelDatabaseSystem, ModelSerializationError> {
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
}
