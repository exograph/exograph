use introspection_resolver::IntrospectionResolver;
use thiserror::Error;

use core_plugin_interface::interface::SubsystemLoader;
use core_plugin_interface::interface::{LibraryLoadingError, SubsystemLoadingError};
use core_plugin_shared::{
    error::ModelSerializationError, serializable_system::SerializableSystem,
    system_serializer::SystemSerializer,
};

use core_resolver::plugin::SubsystemResolver;
use core_resolver::{introspection::definition::schema::Schema, system_resolver::SystemResolver};
use tracing::debug;
pub struct SystemLoader;

impl SystemLoader {
    pub fn load(
        read: impl std::io::Read,
        static_loaders: Vec<Box<dyn SubsystemLoader>>,
    ) -> Result<SystemResolver, SystemLoadingError> {
        let serialized_system = SerializableSystem::deserialize_reader(read)
            .map_err(SystemLoadingError::ModelSerializationError)?;

        Self::process(serialized_system, static_loaders)
    }

    pub fn load_from_bytes(
        bytes: Vec<u8>,
        static_loaders: Vec<Box<dyn SubsystemLoader>>,
    ) -> Result<SystemResolver, SystemLoadingError> {
        let serialized_system = SerializableSystem::deserialize(bytes)
            .map_err(SystemLoadingError::ModelSerializationError)?;

        Self::process(serialized_system, static_loaders)
    }

    fn process(
        serialized_system: SerializableSystem,
        mut static_loaders: Vec<Box<dyn SubsystemLoader>>,
    ) -> Result<SystemResolver, SystemLoadingError> {
        let SerializableSystem {
            subsystems,
            query_interception_map,
            mutation_interception_map,
        } = serialized_system;

        // First build subsystem resolvers
        let subsystem_resolvers: Result<Vec<_>, _> = subsystems
            .into_iter()
            .map(|serialized_subsystem| {
                let subsystem_id = serialized_subsystem.id;
                // First try to load a static loader
                let index = static_loaders
                    .iter()
                    .position(|loader| loader.id() == subsystem_id);

                let subsystem_loader = match index {
                    Some(index) => {
                        debug!("Using static loader for {}", subsystem_id);
                        static_loaders.remove(index)
                    }
                    None => {
                        // Then try to load a dynamic loader
                        debug!("Using dynamic loader for {}", subsystem_id);
                        let subsystem_library_name = format!("{subsystem_id}_resolver_dynamic");

                        core_plugin_interface::interface::load_subsystem_loader(
                            &subsystem_library_name,
                        )?
                    }
                };

                subsystem_loader
                    .init(serialized_subsystem.serialized_subsystem)
                    .map_err(SystemLoadingError::SubsystemLoadingError)
            })
            .collect();

        let mut subsystem_resolvers = subsystem_resolvers?;

        // Then use those resolvers to build the schema
        let schema = Schema::new_from_resolvers(&subsystem_resolvers);

        if let Some(introspection_resolver) =
            Self::create_introspection_resolver(&subsystem_resolvers)?
        {
            subsystem_resolvers.push(introspection_resolver);
        }

        let (normal_query_depth_limit, introspection_query_depth_limit) = query_depth_limits()?;

        Ok(SystemResolver::new(
            subsystem_resolvers,
            query_interception_map,
            mutation_interception_map,
            schema,
            normal_query_depth_limit,
            introspection_query_depth_limit,
        ))
    }

    fn create_introspection_resolver(
        subsystem_resolvers: &[Box<dyn SubsystemResolver + Send + Sync>],
    ) -> Result<Option<Box<IntrospectionResolver>>, SystemLoadingError> {
        let schema = Schema::new_from_resolvers(subsystem_resolvers);

        allow_introspection().map(|allow_introspection| {
            if allow_introspection {
                Some(Box::new(IntrospectionResolver::new(schema)))
            } else {
                None
            }
        })
    }
}

pub fn allow_introspection() -> Result<bool, SystemLoadingError> {
    match std::env::var("EXO_INTROSPECTION").ok() {
        Some(e) => match e.parse::<bool>() {
            Ok(v) => Ok(v),
            Err(_) => Err(SystemLoadingError::Config(
                "EXO_INTROSPECTION env var must be set to either true or false".into(),
            )),
        },
        None => Ok(false),
    }
}

/// Returns the maximum depth of a selection set for normal queries and introspection queries. We
/// hard-code the introspection query depth to 15 to accomodate the query invoked by GraphQL
/// Playground
pub fn query_depth_limits() -> Result<(usize, usize), SystemLoadingError> {
    const DEFAULT_QUERY_DEPTH: usize = 5;
    const DEFAULT_INTROSPECTION_QUERY_DEPTH: usize = 15;

    let query_depth = match std::env::var("EXO_MAX_SELECTION_DEPTH").ok() {
        Some(e) => match e.parse::<usize>() {
            Ok(v) => Ok(v),
            Err(_) => Err(SystemLoadingError::Config(
                "EXO_MAX_SELECTION_DEPTH env var must be set to a positive integer".into(),
            )),
        },
        None => Ok(DEFAULT_QUERY_DEPTH),
    }?;

    Ok((query_depth, DEFAULT_INTROSPECTION_QUERY_DEPTH))
}

#[derive(Error, Debug)]
pub enum SystemLoadingError {
    #[error("System serialization error: {0}")]
    ModelSerializationError(#[from] ModelSerializationError),

    #[error("Error while trying to load subsystem library: {0}")]
    LibraryLoadingError(#[from] LibraryLoadingError),

    #[error("Subsystem loading error: {0}")]
    SubsystemLoadingError(#[from] SubsystemLoadingError),

    #[error("No such file {0}")]
    FileNotFound(String),

    #[error("Failed to open file {0}")]
    FileOpen(String, #[source] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}
