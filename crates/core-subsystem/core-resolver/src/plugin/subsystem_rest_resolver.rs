use async_trait::async_trait;
use common::http::{RequestPayload, ResponsePayload};

use super::SubsystemResolutionError;

#[async_trait]
pub trait SubsystemRestResolver: Sync {
    /// The id of the subsystem (for debugging purposes)
    fn id(&self) -> &'static str;

    async fn resolve(
        &self,
        request: &(dyn RequestPayload + Send + Sync),
    ) -> Result<Option<ResponsePayload>, SubsystemResolutionError>;
}
