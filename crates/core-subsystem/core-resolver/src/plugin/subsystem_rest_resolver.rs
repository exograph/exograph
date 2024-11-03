use async_trait::async_trait;

#[async_trait]
pub trait SubsystemRestResolver: Sync {
    /// The id of the subsystem (for debugging purposes)
    fn id(&self) -> &'static str;

    // async fn resolve(&self, request: Request) -> Result<Response, SubsystemResolutionError>;
}
