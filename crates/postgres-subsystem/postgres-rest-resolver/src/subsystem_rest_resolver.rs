use std::sync::Arc;

use async_trait::async_trait;

use common::http::{RequestPayload, ResponsePayload};
use core_plugin_interface::core_resolver::plugin::{
    SubsystemResolutionError, SubsystemRestResolver,
};
use exo_sql::DatabaseExecutor;
use postgres_rest_model::{
    operation::PostgresOperation, subsystem::PostgresRestSubsystemWithRouter,
};

pub struct PostgresSubsystemRestResolver {
    #[allow(dead_code)]
    pub id: &'static str,
    pub subsystem: PostgresRestSubsystemWithRouter,
    #[allow(dead_code)]
    pub executor: Arc<DatabaseExecutor>,
    pub api_path_prefix: String,
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
        let operation = self
            .subsystem
            .find_matching(request.get_head(), &self.api_path_prefix);

        if let Some(operation) = operation {
            return operation.resolve(request, &self.executor).await;
        }

        Ok(None)
    }
}

#[async_trait]
trait OperationResolver {
    async fn resolve(
        &self,
        request: &(dyn RequestPayload + Send + Sync),
        executor: &DatabaseExecutor,
    ) -> Result<Option<ResponsePayload>, SubsystemResolutionError>;
}

#[async_trait]
impl OperationResolver for PostgresOperation {
    async fn resolve(
        &self,
        request: &(dyn RequestPayload + Send + Sync),
        _executor: &DatabaseExecutor,
    ) -> Result<Option<ResponsePayload>, SubsystemResolutionError> {
        todo!(
            "Resolve: {:?} {:?}",
            request.get_head().get_method(),
            request.get_head().get_path()
        )
    }
}
