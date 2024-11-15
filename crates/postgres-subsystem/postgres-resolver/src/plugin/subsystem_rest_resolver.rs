use std::sync::Arc;

use async_trait::async_trait;

use common::http::{RequestPayload, ResponsePayload};
use core_plugin_interface::core_resolver::plugin::{
    SubsystemResolutionError, SubsystemRestResolver,
};
use exo_sql::DatabaseExecutor;
use postgres_rest_model::subsystem::PostgresRestSubsystem;

pub struct PostgresSubsystemRestResolver {
    #[allow(dead_code)]
    pub id: &'static str,
    #[allow(dead_code)]
    pub subsystem: PostgresRestSubsystem,
    #[allow(dead_code)]
    pub executor: Arc<DatabaseExecutor>,
}

#[async_trait]
impl SubsystemRestResolver for PostgresSubsystemRestResolver {
    fn id(&self) -> &'static str {
        "postgres"
    }

    async fn resolve(
        &self,
        request: &(dyn RequestPayload + Send + Sync),
    ) -> Result<Option<ResponsePayload>, SubsystemResolutionError> {
        todo!(
            "PostgresSubsystemRestResolver: {}",
            request.get_head().get_path()
        )
    }
}
