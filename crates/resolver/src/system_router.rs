use async_trait::async_trait;
use core_resolver::{
    http::{RequestPayload, ResponsePayload},
    system_resolver::SystemRouter,
};

pub struct SystemRouter {
    // pub system_resolvers: Vec<SystemResolver>,
    subsystem_routers: Vec<Box<dyn SubsystemRouter + Send + Sync>>,
}

impl SystemRouter {
    pub fn new(subsystem_routers: Vec<Box<dyn SubsystemRouter + Send + Sync>>) -> Self {
        Self { subsystem_routers }
    }

    pub async fn resolve<'a, E: 'static>(
        &self,
        request: impl RequestPayload,
        playground_request: bool,
    ) -> ResponsePayload {
        // super::resolve(request, &self.system_resolver, playground_request).await
        todo!()
    }
}

#[async_trait]
pub trait SubsystemRouter {
    async fn resolve<'a>(
        &self,
        request: Box<dyn RequestPayload>,
        playground_request: bool,
    ) -> Option<ResponsePayload>;
}

#[async_trait]
impl SubsystemRouter for SystemRouter {
    async fn resolve<'a>(
        &self,
        request: Box<dyn RequestPayload>,
        playground_request: bool,
    ) -> Option<ResponsePayload> {
        Some(super::resolve(request, self, playground_request).await)
    }
}
