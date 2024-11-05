use std::{fs::File, io::BufReader, path::Path, sync::Arc};

use tracing::debug;

use common::{
    cors::{CorsConfig, CorsRouter},
    env_const::{EXO_CORS_DOMAINS, EXO_UNSTABLE_ENABLE_REST_API},
    http::{RequestPayload, ResponsePayload},
    router::{CompositeRouter, Router},
};
use core_plugin_interface::{
    core_resolver::{
        context::JwtAuthenticator,
        plugin::{SubsystemGraphQLResolver, SubsystemRestResolver},
        system_rest_resolver::SystemRestResolver,
    },
    interception::InterceptionMap,
    interface::{SubsystemLoader, SubsystemResolver},
    serializable_system::SerializableSystem,
    system_serializer::SystemSerializer,
    trusted_documents::TrustedDocuments,
};
use exo_env::Environment;
use graphql_router::{GraphQLRouter, SystemLoader, SystemLoadingError};

#[cfg(not(target_family = "wasm"))]
use playground_router::PlaygroundRouter;
use rest_router::RestRouter;

pub type StaticLoaders = Vec<Box<dyn SubsystemLoader>>;

pub async fn create_system_router_from_file(
    exo_ir_file: &str,
    static_loaders: StaticLoaders,
    env: Arc<dyn Environment>,
) -> Result<SystemRouter, SystemLoadingError> {
    if !Path::new(&exo_ir_file).exists() {
        return Err(SystemLoadingError::FileNotFound(exo_ir_file.to_string()));
    }

    match File::open(exo_ir_file) {
        Ok(file) => {
            let exo_ir_file_buffer = BufReader::new(file);

            let serialized_system = SerializableSystem::deserialize_reader(exo_ir_file_buffer)
                .map_err(SystemLoadingError::ModelSerializationError)?;

            create_system_router_from_system(serialized_system, static_loaders, env).await
        }
        Err(e) => Err(SystemLoadingError::FileOpen(exo_ir_file.into(), e)),
    }
}

pub async fn create_system_router_from_system(
    system: SerializableSystem,
    static_loaders: StaticLoaders,
    env: Arc<dyn Environment>,
) -> Result<SystemRouter, SystemLoadingError> {
    let authenticator = JwtAuthenticator::new_from_env(env.as_ref())
        .await
        .map_err(|e| SystemLoadingError::Config(e.to_string()))?;

    let (subsystem_resolvers, query_interception_map, mutation_interception_map, trusted_documents) =
        create_system_resolvers(system, static_loaders, env.clone()).await?;

    let mut graphql_resolvers: Vec<Box<dyn SubsystemGraphQLResolver + Send + Sync>> = vec![];
    let mut rest_resolvers: Vec<Box<dyn SubsystemRestResolver + Send + Sync>> = vec![];

    for resolver in subsystem_resolvers {
        let SubsystemResolver { graphql, rest } = *resolver;

        if let Some(graphql) = graphql {
            graphql_resolvers.push(graphql);
        }

        if let Some(rest) = rest {
            rest_resolvers.push(rest);
        }
    }

    let graphql_resolver = SystemLoader::create_system_resolver(
        graphql_resolvers,
        query_interception_map,
        mutation_interception_map,
        trusted_documents,
        authenticator.into(),
        env.clone(),
    )
    .await?;

    let graphql_router = GraphQLRouter::new(graphql_resolver, env.clone());

    let rest_resolver = SystemRestResolver::new(rest_resolvers, env.clone());
    let rest_router = RestRouter::new(rest_resolver, env.clone());

    create_system_router_from_resolver(graphql_router, rest_router, env)
}

pub async fn create_system_resolvers(
    system: SerializableSystem,
    mut static_loaders: StaticLoaders,
    env: Arc<dyn Environment>,
) -> Result<
    (
        Vec<Box<SubsystemResolver>>,
        InterceptionMap,
        InterceptionMap,
        TrustedDocuments,
    ),
    SystemLoadingError,
> {
    fn get_loader(
        static_loaders: &mut StaticLoaders,
        subsystem_id: String,
    ) -> Result<Box<dyn SubsystemLoader>, SystemLoadingError> {
        // First try to find a static loader
        let static_loader = {
            let index = static_loaders
                .iter()
                .position(|loader| loader.id() == subsystem_id);

            index.map(|index| static_loaders.remove(index))
        };

        if let Some(loader) = static_loader {
            debug!("Using static loader for {}", subsystem_id);
            Ok(loader)
        } else {
            #[cfg(not(target_family = "wasm"))]
            {
                // Otherwise try to load a dynamic loader
                debug!("Using dynamic loader for {}", subsystem_id);
                let subsystem_library_name = format!("{subsystem_id}_resolver_dynamic");

                let loader = core_plugin_interface::interface::load_subsystem_loader(
                    &subsystem_library_name,
                )?;
                Ok(loader)
            }

            #[cfg(target_family = "wasm")]
            {
                panic!("Dynamic loading is not supported on WASM");
            }
        }
    }

    let mut subsystem_resolvers: Vec<Box<SubsystemResolver>> = vec![];

    let SerializableSystem {
        subsystems,
        query_interception_map,
        mutation_interception_map,
        trusted_documents,
    } = system;

    for subsystem in subsystems {
        let mut loader = get_loader(&mut static_loaders, subsystem.id.clone())?;

        let resolver = loader
            .init(subsystem, env.as_ref())
            .await
            .map_err(SystemLoadingError::SubsystemLoadingError)?;

        subsystem_resolvers.push(resolver);
    }

    Ok((
        subsystem_resolvers,
        query_interception_map,
        mutation_interception_map,
        trusted_documents,
    ))
}

fn create_system_router_from_resolver(
    graphql_router: GraphQLRouter,
    rest_router: RestRouter,
    env: Arc<dyn Environment>,
) -> Result<SystemRouter, SystemLoadingError> {
    let mut routers: Vec<Box<dyn Router + Send>> = vec![Box::new(graphql_router)];

    if env.enabled(EXO_UNSTABLE_ENABLE_REST_API, false) {
        routers.push(Box::new(rest_router));
    }

    #[cfg(not(target_family = "wasm"))]
    routers.push(Box::new(PlaygroundRouter::new(env.clone())));

    Ok(SystemRouter::new(routers, env.as_ref()))
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
