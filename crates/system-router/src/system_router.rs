// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{fs::File, io::BufReader, path::Path, sync::Arc};

use common::env_const::{EXO_ENABLE_MCP, EXO_UNSTABLE_ENABLE_RPC_API};
use common::introspection::{IntrospectionMode, introspection_mode};
use common::router::PlainRequestPayload;
use core_plugin_shared::profile::{SchemaProfile, SchemaProfiles};
use core_resolver::introspection::definition::schema::Schema;
use core_resolver::plugin::SubsystemRpcResolver;
use core_resolver::system_rpc_resolver::SystemRpcResolver;
use core_resolver::{
    plugin::{SubsystemGraphQLResolver, SubsystemRestResolver},
    system_rest_resolver::SystemRestResolver,
};

#[cfg(not(target_family = "wasm"))]
use mcp_router::McpRouter;
use rpc_router::RpcRouter;
use tracing::debug;

use common::context::{JwtAuthenticator, RequestContext};
use common::{
    cors::{CorsConfig, CorsRouter},
    env_const::{EXO_CORS_DOMAINS, EXO_GRAPHQL_ALLOW_MUTATIONS, EXO_UNSTABLE_ENABLE_REST_API},
    http::ResponsePayload,
    router::{CompositeRouter, Router},
};
use core_plugin_interface::interface::{SubsystemLoader, SubsystemResolver};
use core_plugin_shared::{
    interception::InterceptionMap, serializable_system::SerializableSystem,
    system_serializer::SystemSerializer, trusted_documents::TrustedDocuments,
};
use core_router::SystemLoadingError;
use exo_env::Environment;
use graphql_router::{GraphQLRouter, IntrospectionResolver};

#[cfg(not(target_family = "wasm"))]
use playground_router::PlaygroundRouter;
#[cfg(not(target_family = "wasm"))]
use playground_router::PlaygroundRouterConfig;

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
    let (
        subsystem_resolvers,
        query_interception_map,
        mutation_interception_map,
        trusted_documents,
        declaration_doc_comments,
        schema_profiles,
    ) = create_system_resolvers(system, static_loaders, env.clone()).await?;

    let query_interception_map = Arc::new(query_interception_map);
    let mutation_interception_map = Arc::new(mutation_interception_map);

    let declaration_doc_comments = Arc::new(declaration_doc_comments);

    let mut graphql_resolvers: Vec<Arc<dyn SubsystemGraphQLResolver + Send + Sync>> = vec![];
    let mut rest_resolvers: Vec<Box<dyn SubsystemRestResolver + Send + Sync>> = vec![];
    let mut rpc_resolvers: Vec<Box<dyn SubsystemRpcResolver + Send + Sync>> = vec![];

    for resolver in subsystem_resolvers {
        let SubsystemResolver { graphql, rest, rpc } = *resolver;

        if let Some(graphql) = graphql {
            graphql_resolvers.push(graphql);
        }

        if let Some(rest) = rest {
            rest_resolvers.push(rest);
        }

        if let Some(rpc) = rpc {
            rpc_resolvers.push(rpc);
        }
    }

    let graphql_router = {
        let allow_mutations = env.enabled(EXO_GRAPHQL_ALLOW_MUTATIONS, true)?;

        let profile = if allow_mutations {
            SchemaProfile::all()
        } else {
            SchemaProfile::queries_only()
        };

        let introspection_schema = Arc::new(Schema::new_from_resolvers(
            &graphql_resolvers,
            &profile,
            declaration_doc_comments.clone(),
        ));

        let (introspection_resolver, graphql_resolvers): (
            Option<Arc<dyn SubsystemGraphQLResolver + Send + Sync>>,
            _,
        ) = match introspection_mode(env.as_ref())? {
            IntrospectionMode::Disabled => (None, graphql_resolvers.clone()),
            IntrospectionMode::Enabled => {
                let introspection_resolver =
                    Arc::new(IntrospectionResolver::new(introspection_schema.clone()));
                (Some(introspection_resolver), graphql_resolvers.clone())
            }
            IntrospectionMode::Only => {
                // forgo all other resolvers and only use introspection
                let introspection_resolver =
                    Arc::new(IntrospectionResolver::new(introspection_schema.clone()));
                (Some(introspection_resolver), vec![])
            }
        };

        GraphQLRouter::from_resolvers(
            graphql_resolvers.to_vec(),
            introspection_resolver,
            introspection_schema,
            query_interception_map.clone(),
            mutation_interception_map.clone(),
            trusted_documents,
            env.clone(),
        )?
    };

    let rest_resolver = SystemRestResolver::new(rest_resolvers, env.clone());
    let rest_router = RestRouter::new(rest_resolver, env.clone());

    let rpc_resolver = SystemRpcResolver::new(rpc_resolvers, env.clone());
    let rpc_router = RpcRouter::new(rpc_resolver, env.clone());

    #[cfg(not(target_family = "wasm"))]
    let mcp_router = create_mcp_router(
        env.clone(),
        graphql_resolvers,
        declaration_doc_comments,
        query_interception_map,
        mutation_interception_map,
        schema_profiles,
    )
    .await?;

    #[cfg(not(target_family = "wasm"))]
    {
        create_system_router(graphql_router, rest_router, rpc_router, mcp_router, env).await
    }

    #[cfg(target_family = "wasm")]
    {
        create_system_router(graphql_router, rest_router, rpc_router, env).await
    }
}

#[cfg(not(target_family = "wasm"))]
async fn create_mcp_router(
    env: Arc<dyn Environment>,
    graphql_resolvers: Vec<Arc<dyn SubsystemGraphQLResolver + Send + Sync>>,
    declaration_doc_comments: Arc<Option<String>>,
    query_interception_map: Arc<InterceptionMap>,
    mutation_interception_map: Arc<InterceptionMap>,
    schema_profiles: Option<SchemaProfiles>,
) -> Result<McpRouter, SystemLoadingError> {
    let env_clone = env.clone();
    let declaration_doc_comments_clone = declaration_doc_comments.clone();

    let create_resolver = |profile: &SchemaProfile| {
        let introspection_schema = Arc::new(Schema::new_from_resolvers(
            &graphql_resolvers,
            profile,
            declaration_doc_comments_clone.clone(),
        ));

        let introspection_resolver =
            Arc::new(IntrospectionResolver::new(introspection_schema.clone()));

        let graphql_router = GraphQLRouter::from_resolvers(
            graphql_resolvers.clone(),
            Some(introspection_resolver.clone()),
            introspection_schema.clone(),
            query_interception_map.clone(),
            mutation_interception_map.clone(),
            TrustedDocuments::all(),
            env.clone(),
        )?;

        Ok(graphql_router.resolver())
    };

    McpRouter::new(env_clone, create_resolver, schema_profiles)
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
        Option<String>,
        Option<SchemaProfiles>,
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
        declaration_doc_comments,
        schema_profiles,
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
        declaration_doc_comments,
        schema_profiles,
    ))
}

async fn create_system_router(
    graphql_router: GraphQLRouter,
    rest_router: RestRouter,
    rpc_router: RpcRouter,
    #[cfg(not(target_family = "wasm"))] mcp_router: McpRouter,
    env: Arc<dyn Environment>,
) -> Result<SystemRouter, SystemLoadingError> {
    let mut routers: Vec<Box<dyn for<'a> Router<RequestContext<'a>> + Send + Sync>> =
        vec![Box::new(graphql_router)];

    if env.enabled(EXO_UNSTABLE_ENABLE_REST_API, false)? {
        routers.push(Box::new(rest_router));
    }

    if env.enabled(EXO_UNSTABLE_ENABLE_RPC_API, false)? {
        routers.push(Box::new(rpc_router));
    }

    #[cfg(not(target_family = "wasm"))]
    {
        if env.enabled(EXO_ENABLE_MCP, true)? {
            routers.push(Box::new(mcp_router));
        }
    }

    #[cfg(target_family = "wasm")]
    {
        SystemRouter::new(routers, env.clone()).await
    }

    #[cfg(not(target_family = "wasm"))]
    {
        let playground_config = Arc::new(PlaygroundRouterConfig::new(env.clone()));

        routers.push(Box::new(PlaygroundRouter::new(playground_config.clone())));

        SystemRouter::new(routers, env.clone(), Some(playground_config)).await
    }
}

type RequestContextRouter = Box<dyn for<'b> Router<RequestContext<'b>> + Send + Sync>;

pub struct SystemRouter {
    underlying: CorsRouter<CompositeRouter<RequestContextRouter>>,
    env: Arc<dyn Environment>,
    authenticator: Arc<Option<JwtAuthenticator>>,
    #[cfg(not(target_family = "wasm"))]
    playground_config: Option<Arc<PlaygroundRouterConfig>>,
}

impl SystemRouter {
    pub async fn new(
        routers: Vec<RequestContextRouter>,
        env: Arc<dyn Environment>,
        #[cfg(not(target_family = "wasm"))] playground_config: Option<Arc<PlaygroundRouterConfig>>,
    ) -> Result<Self, SystemLoadingError> {
        let cors_domains = env.get(EXO_CORS_DOMAINS);

        let authenticator = JwtAuthenticator::new_from_env(env.as_ref())
            .await
            .map_err(|e| SystemLoadingError::Config(e.to_string()))?;

        Ok(Self {
            underlying: CorsRouter::new(
                CompositeRouter::new(routers),
                CorsConfig::from_env(cors_domains),
            ),
            env,
            authenticator: Arc::new(authenticator),
            #[cfg(not(target_family = "wasm"))]
            playground_config,
        })
    }

    pub fn is_playground_assets_request(
        &self,
        request_path: &str,
        request_method: http::Method,
    ) -> bool {
        #[cfg(target_family = "wasm")]
        {
            false
        }

        #[cfg(not(target_family = "wasm"))]
        {
            if let Some(playground_config) = &self.playground_config {
                playground_config.suitable(request_path, request_method)
            } else {
                false
            }
        }
    }
}

#[async_trait::async_trait]
impl<'request> Router<PlainRequestPayload<'request>> for SystemRouter {
    async fn route(
        &self,
        request_context: &PlainRequestPayload<'request>,
    ) -> Option<ResponsePayload> {
        match request_context {
            PlainRequestPayload::External(request) => {
                let request_context = RequestContext::new(
                    request.as_ref(),
                    vec![],
                    self,
                    &self.authenticator,
                    self.env.as_ref(),
                );

                self.underlying.route(&request_context).await
            }
            PlainRequestPayload::Internal(request_context) => {
                self.underlying.route(request_context).await
            }
        }
    }
}
