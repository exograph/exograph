use async_trait::async_trait;
use common::context::RequestContext;
use common::http::ResponsePayload;

use super::SubsystemResolutionError;

#[async_trait]
pub trait SubsystemRestResolver: Sync {
    /// The id of the subsystem (for debugging purposes)
    fn id(&self) -> &'static str;

    async fn resolve<'a>(
        &self,
        request_context: &'a RequestContext<'a>,
    ) -> Result<Option<ResponsePayload>, SubsystemResolutionError>;
}
