use payas_introspection_resolver::IntrospectionResolver;
use thiserror::Error;

use payas_core_model::{
    error::ModelSerializationError, serializable_system::SerializableSystem,
    system_serializer::SystemSerializer,
};
use payas_core_resolver::{
    introspection::definition::schema::Schema,
    plugin::{SubsystemLoader, SubsystemLoadingError, SubsystemResolver},
    system_resolver::SystemResolver,
};
use payas_database_resolver::DatabaseSubsystemLoader;
use payas_deno_resolver::DenoSubsystemLoader;
use payas_wasm_resolver::WasmSubsystemLoader;

pub struct SystemLoader;

impl SystemLoader {
    pub fn load(read: impl std::io::Read) -> Result<SystemResolver, SystemLoadingError> {
        let serialized_system = SerializableSystem::deserialize_reader(read)
            .map_err(SystemLoadingError::ModelSerializationError)?;

        Self::process(serialized_system)
    }

    pub fn load_from_bytes(bytes: Vec<u8>) -> Result<SystemResolver, SystemLoadingError> {
        let serialized_system = SerializableSystem::deserialize(bytes)
            .map_err(SystemLoadingError::ModelSerializationError)?;

        Self::process(serialized_system)
    }

    fn process(
        serialized_system: SerializableSystem,
    ) -> Result<SystemResolver, SystemLoadingError> {
        let SerializableSystem {
            subsystems,
            query_interception_map,
            mutation_interception_map,
        } = serialized_system;

        let database_loader = DatabaseSubsystemLoader {};
        let deno_loader = DenoSubsystemLoader {};
        let wasm_loader = WasmSubsystemLoader {};
        let loaders: Vec<&dyn SubsystemLoader> = vec![&database_loader, &deno_loader, &wasm_loader];

        let subsystem_resolvers: Result<Vec<_>, _> = subsystems
            .into_iter()
            .map(|serialized_subsystem| {
                let subsystem_loader = loaders
                    .iter()
                    .find(|loader| loader.id() == serialized_subsystem.id)
                    .ok_or_else(|| {
                        SystemLoadingError::SubsystemLoaderNotFound(serialized_subsystem.id.clone())
                    })?;

                subsystem_loader
                    .init(serialized_subsystem.serialized_subsystem)
                    .map_err(SystemLoadingError::SubsystemLoadingError)
            })
            .collect();

        let mut subsystem_resolvers = subsystem_resolvers?;
        let schema = Schema::new(&subsystem_resolvers);

        if let Some(introspection_resolver) =
            Self::create_introspection_resolver(&subsystem_resolvers)?
        {
            subsystem_resolvers.push(introspection_resolver);
        }

        Ok(SystemResolver::new(
            subsystem_resolvers,
            query_interception_map,
            mutation_interception_map,
            schema,
        ))
    }

    fn create_introspection_resolver(
        subsystem_resolvers: &Vec<Box<dyn SubsystemResolver + Send + Sync>>,
    ) -> Result<Option<Box<IntrospectionResolver>>, SystemLoadingError> {
        let schema = Schema::new(subsystem_resolvers);

        let allow_introspection = match std::env::var("CLAY_INTROSPECTION").ok() {
            Some(e) => match e.parse::<bool>() {
                Ok(v) => Ok(v),
                Err(_) => Err(SystemLoadingError::Config(
                    "CLAY_INTROSPECTION env var must be set to either true or false".into(),
                )),
            },
            None => Ok(false),
        };

        allow_introspection.map(|allow_introspection| {
            if allow_introspection {
                Some(Box::new(IntrospectionResolver::new(schema)))
            } else {
                None
            }
        })
    }
}

#[derive(Error, Debug)]
pub enum SystemLoadingError {
    #[error("System serialization error: {0}")]
    ModelSerializationError(#[from] ModelSerializationError),

    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("Subsystem loader for '{0}' not found")]
    SubsystemLoaderNotFound(String),

    #[error("Subsystem loading error: {0}")]
    SubsystemLoadingError(#[from] SubsystemLoadingError),

    #[error("No such file {0}")]
    FileNotFound(String),

    #[error("Failed to open file {0}")]
    FileOpen(String, #[source] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}
