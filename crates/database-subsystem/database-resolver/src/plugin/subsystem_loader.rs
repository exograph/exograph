use core_plugin::system_serializer::SystemSerializer;
use core_resolver::plugin::{SubsystemLoader, SubsystemLoadingError, SubsystemResolver};
use database_model::model::ModelDatabaseSystem;
use payas_sql::{Database, DatabaseExecutor};

use super::DatabaseSubsystemResolver;

pub struct DatabaseSubsystemLoader {}

impl SubsystemLoader for DatabaseSubsystemLoader {
    fn id(&self) -> &'static str {
        "database"
    }

    fn init<'a>(
        &self,
        serialized_subsystem: Vec<u8>,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError> {
        let subsystem = ModelDatabaseSystem::deserialize(serialized_subsystem)?;

        let database =
            Database::from_env(None).map_err(|e| SubsystemLoadingError::BoxedError(Box::new(e)))?;
        let executor = DatabaseExecutor { database };

        Ok(Box::new(DatabaseSubsystemResolver {
            id: self.id(),
            subsystem,
            executor,
        }))
    }
}
