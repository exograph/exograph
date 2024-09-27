use std::sync::Arc;

use common::{
    api_router::ApiRouter,
    http::{RequestPayload, ResponsePayload},
    introspection::{introspection_mode, IntrospectionMode},
};
use core_plugin_interface::serializable_system::SerializableSystem;
use exo_env::Environment;
use http::StatusCode;
use resolver::{
    create_system_resolver, create_system_resolver_from_system, GraphQLRouter, StaticLoaders,
    SystemLoadingError,
};

pub struct SystemRouter {
    // TODO: add other routers here (or use a vector of routers)
    graphql_router: GraphQLRouter,
    env: Arc<dyn Environment>,
}

impl SystemRouter {
    pub async fn new_from_file(
        exo_ir_file: &str,
        static_loaders: StaticLoaders,
        env: Arc<dyn Environment>,
    ) -> Result<Self, SystemLoadingError> {
        let resolver = create_system_resolver(exo_ir_file, static_loaders, env.clone()).await?;

        Ok(Self {
            graphql_router: GraphQLRouter::new(resolver),
            env: env.clone(),
        })
    }

    pub async fn new_from_system(
        system: SerializableSystem,
        static_loaders: StaticLoaders,
        env: Arc<dyn Environment>,
    ) -> Result<Self, SystemLoadingError> {
        let resolver =
            create_system_resolver_from_system(system, static_loaders, env.clone()).await?;

        Ok(Self {
            graphql_router: GraphQLRouter::new(resolver),
            env: env.clone(),
        })
    }

    pub async fn route(
        &self,
        request: impl RequestPayload + Send,
        playground_request: bool,
    ) -> ResponsePayload {
        if self.graphql_router.suitable(request.get_head()).await {
            ApiRouter::route(&self.graphql_router, request, playground_request).await
        } else {
            ResponsePayload {
                stream: None,
                headers: vec![],
                status_code: StatusCode::NOT_FOUND,
            }
        }
    }

    /// Should we allow introspection queries?
    pub fn allow_introspection(&self) -> bool {
        introspection_mode(self.env.as_ref()).unwrap_or(IntrospectionMode::Disabled)
            == IntrospectionMode::Enabled
    }
}
