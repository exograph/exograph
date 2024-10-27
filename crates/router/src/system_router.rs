use std::sync::Arc;

use common::{
    cors::CorsConfig,
    cors::CorsRouter,
    env_const::EXO_CORS_DOMAINS,
    http::{RequestPayload, ResponsePayload},
    router::{CompositeRouter, Router},
};
use core_plugin_interface::{
    core_resolver::system_resolver::SystemResolver, serializable_system::SerializableSystem,
};
use exo_env::Environment;
use graphql_router::{
    create_system_resolver, create_system_resolver_from_system, GraphQLRouter, StaticLoaders,
    SystemLoadingError,
};
#[cfg(not(target_family = "wasm"))]
use playground_router::PlaygroundRouter;
use rest_router::RestRouter;

pub async fn create_system_router_from_file(
    exo_ir_file: &str,
    static_loaders: StaticLoaders,
    env: Arc<dyn Environment>,
) -> Result<SystemRouter, SystemLoadingError> {
    let resolver = create_system_resolver(exo_ir_file, static_loaders, env.clone()).await?;

    create_system_router_from_resolver(resolver, env)
}

pub async fn create_system_router_from_system(
    system: SerializableSystem,
    static_loaders: StaticLoaders,
    env: Arc<dyn Environment>,
) -> Result<SystemRouter, SystemLoadingError> {
    let resolver = create_system_resolver_from_system(system, static_loaders, env.clone()).await?;

    create_system_router_from_resolver(resolver, env)
}

fn create_system_router_from_resolver(
    resolver: SystemResolver,
    env: Arc<dyn Environment>,
) -> Result<SystemRouter, SystemLoadingError> {
    Ok(SystemRouter::new(
        vec![
            Box::new(GraphQLRouter::new(resolver, env.clone())),
            Box::new(RestRouter::new(env.clone())),
            #[cfg(not(target_family = "wasm"))]
            Box::new(PlaygroundRouter::new(env.clone())),
        ],
        env.as_ref(),
    ))
}

pub struct SystemRouter {
    underlying: CorsRouter,
}

impl SystemRouter {
    pub fn new(routers: Vec<Box<dyn Router + Send>>, env: &dyn Environment) -> Self {
        let cors_domains = env.get(EXO_CORS_DOMAINS);

        Self {
            underlying: CorsRouter::new(
                Arc::new(CompositeRouter::new(routers)),
                CorsConfig::from_env(cors_domains),
            ),
        }
    }
}

#[async_trait::async_trait]
impl Router for SystemRouter {
    async fn route(&self, request: &mut (dyn RequestPayload + Send)) -> Option<ResponsePayload> {
        self.underlying.route(request).await
    }
}
