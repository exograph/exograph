use super::PostgresSubsystemResolver;
use core_plugin_interface::{
    core_resolver::plugin::SubsystemResolver,
    interface::{SubsystemLoader, SubsystemLoadingError},
    system_serializer::SystemSerializer,
};
use payas_sql::{Database, DatabaseExecutor};
use postgres_model::model::ModelPostgresSystem;

pub struct PostgresSubsystemLoader {}
core_plugin_interface::export_subsystem_loader!(PostgresSubsystemLoader {});

impl SubsystemLoader for PostgresSubsystemLoader {
    fn id(&self) -> &'static str {
        "postgres"
    }

    fn init<'a>(
        &self,
        serialized_subsystem: Vec<u8>,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError> {
        let subsystem = ModelPostgresSystem::deserialize(serialized_subsystem)?;

        let database =
            Database::from_env(None).map_err(|e| SubsystemLoadingError::BoxedError(Box::new(e)))?;
        let executor = DatabaseExecutor { database };

        Ok(Box::new(PostgresSubsystemResolver {
            id: self.id(),
            subsystem,
            executor,
        }))
    }
}
