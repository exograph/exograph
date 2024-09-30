use std::sync::Arc;

use common::router::CompositeRouter;
use core_plugin_interface::{
    core_resolver::system_resolver::SystemResolver, serializable_system::SerializableSystem,
};
use exo_env::Environment;
#[cfg(not(target_family = "wasm"))]
use playground_router::PlaygroundRouter;
use resolver::{
    create_system_resolver, create_system_resolver_from_system, GraphQLRouter, StaticLoaders,
    SystemLoadingError,
};

pub async fn create_system_router_from_file(
    exo_ir_file: &str,
    static_loaders: StaticLoaders,
    env: Arc<dyn Environment>,
) -> Result<CompositeRouter, SystemLoadingError> {
    let resolver = create_system_resolver(exo_ir_file, static_loaders, env.clone()).await?;

    create_system_router_from_resolver(resolver, env)
}

pub async fn create_system_router_from_system(
    system: SerializableSystem,
    static_loaders: StaticLoaders,
    env: Arc<dyn Environment>,
) -> Result<CompositeRouter, SystemLoadingError> {
    let resolver = create_system_resolver_from_system(system, static_loaders, env.clone()).await?;

    create_system_router_from_resolver(resolver, env)
}

fn create_system_router_from_resolver(
    resolver: SystemResolver,
    env: Arc<dyn Environment>,
) -> Result<CompositeRouter, SystemLoadingError> {
    Ok(CompositeRouter::new(vec![
        Box::new(GraphQLRouter::new(resolver, env.clone())),
        #[cfg(not(target_family = "wasm"))]
        Box::new(PlaygroundRouter::new(env.clone())),
    ]))
}
