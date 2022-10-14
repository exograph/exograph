use core_plugin::system_serializer::SystemSerializer;
use core_resolver::plugin::{SubsystemLoader, SubsystemLoadingError, SubsystemResolver};
use payas_sql::{Database, DatabaseExecutor};
use postgres_model::model::ModelPostgresSystem;

use super::PostgresSubsystemResolver;

pub struct PostgresSubsystemLoader {}

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
