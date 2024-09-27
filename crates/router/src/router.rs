use common::{
    api_router::ApiRouter,
    http::{RequestPayload, ResponsePayload},
};
use core_plugin_interface::serializable_system::SerializableSystem;
use exo_env::Environment;
use resolver::{
    create_system_resolver, create_system_resolver_from_system, GraphQLRouter, StaticLoaders,
    SystemLoadingError,
};

pub struct SystemRouter {
    // TODO: add other routers here (or use a vector of routers)
    graphql_router: GraphQLRouter,
}

impl SystemRouter {
    pub async fn new_from_file(
        exo_ir_file: &str,
        static_loaders: StaticLoaders,
        env: Box<dyn Environment>,
    ) -> Result<Self, SystemLoadingError> {
        let resolver = create_system_resolver(exo_ir_file, static_loaders, env).await?;

        Ok(Self {
            graphql_router: GraphQLRouter::new(resolver),
        })
    }

    pub async fn new_from_system(
        system: SerializableSystem,
        static_loaders: StaticLoaders,
        env: Box<dyn Environment>,
    ) -> Result<Self, SystemLoadingError> {
        let resolver = create_system_resolver_from_system(system, static_loaders, env).await?;

        Ok(Self {
            graphql_router: GraphQLRouter::new(resolver),
        })
    }

    pub async fn route(
        &self,
        request: impl RequestPayload + Send,
        playground_request: bool,
    ) -> ResponsePayload {
        ApiRouter::route(&self.graphql_router, request, playground_request).await
    }

    pub fn allow_introspection(&self) -> bool {
        self.graphql_router.allow_introspection()
    }
}
