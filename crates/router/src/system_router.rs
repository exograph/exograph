use std::sync::Arc;

use common::{
    api_router::ApiRouter,
    http::{RequestPayload, ResponseBody, ResponsePayload},
};
use core_plugin_interface::{
    core_resolver::system_resolver::SystemResolver, serializable_system::SerializableSystem,
};
use exo_env::Environment;
use http::StatusCode;
use playground_router::PlaygroundRouter;
use resolver::{
    create_system_resolver, create_system_resolver_from_system, GraphQLRouter, StaticLoaders,
    SystemLoadingError,
};

pub struct SystemRouter {
    // TODO: add other routers here (or use a vector of routers)
    graphql_router: GraphQLRouter,
    playground_router: PlaygroundRouter,
}

impl SystemRouter {
    pub async fn new_from_file(
        exo_ir_file: &str,
        static_loaders: StaticLoaders,
        env: Arc<dyn Environment>,
    ) -> Result<Self, SystemLoadingError> {
        let resolver = create_system_resolver(exo_ir_file, static_loaders, env.clone()).await?;

        Self::new_from_resolver(resolver, env)
    }

    pub async fn new_from_system(
        system: SerializableSystem,
        static_loaders: StaticLoaders,
        env: Arc<dyn Environment>,
    ) -> Result<Self, SystemLoadingError> {
        let resolver =
            create_system_resolver_from_system(system, static_loaders, env.clone()).await?;

        Self::new_from_resolver(resolver, env)
    }

    fn new_from_resolver(
        resolver: SystemResolver,
        env: Arc<dyn Environment>,
    ) -> Result<Self, SystemLoadingError> {
        Ok(Self {
            graphql_router: GraphQLRouter::new(resolver, env.clone()),
            playground_router: PlaygroundRouter::new(env.clone()),
        })
    }

    pub async fn route(
        &self,
        request: impl RequestPayload + Send,
        playground_request: bool,
    ) -> ResponsePayload {
        if self.graphql_router.suitable(request.get_head()).await {
            ApiRouter::route(&self.graphql_router, request, playground_request).await
        } else if self.playground_router.suitable(request.get_head()).await {
            ApiRouter::route(&self.playground_router, request, playground_request).await
        } else {
            ResponsePayload {
                body: ResponseBody::None,
                headers: vec![],
                status_code: StatusCode::NOT_FOUND,
            }
        }
    }
}
