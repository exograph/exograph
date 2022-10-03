use payas_core_model::{
    error::ModelSerializationError, serializable_system::SerializableSystem,
    system_serializer::SystemSerializer,
};
use payas_core_resolver::{
    plugin::{SubsystemLoader, SubsystemLoadingError, SubsystemResolver},
    system::SystemResolver,
};
use payas_database_resolver::DatabaseSubsystemLoader;
use thiserror::Error;

pub struct SystemLoader;

impl SystemLoader {
    pub fn load(read: impl std::io::Read) -> Result<SystemResolver, SystemLoadingError> {
        let serialized_system =
            SerializableSystem::deserialize_reader(read).map_err(SystemLoadingError::Init)?;

        Self::process(serialized_system)
    }

    pub fn load_from_bytes(bytes: Vec<u8>) -> Result<SystemResolver, SystemLoadingError> {
        let serialized_system =
            SerializableSystem::deserialize(bytes).map_err(SystemLoadingError::Init)?;

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
        let loaders: Vec<&dyn SubsystemLoader> = vec![&database_loader];

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

        // let introspection_resolver = Self::create_introspection_resolver(&subsystem_resolvers);
        // subsystem_resolvers.push(introspection_resolver);

        Ok(SystemResolver {
            subsystem_resolvers,
            query_interception_map,
            mutation_interception_map,
            allow_introspection: true, // TODO: Fix this
        })
    }

    fn create_introspection_resolver(
        subsystem_resolvers: &Vec<Box<dyn SubsystemResolver>>,
    ) -> Box<dyn SubsystemResolver> {
        todo!()
    }
}

#[derive(Error, Debug)]
pub enum SystemLoadingError {
    #[error("System serialization error: {0}")]
    Init(#[from] ModelSerializationError),

    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("Subsystem loader for '{0}' not found")]
    SubsystemLoaderNotFound(String),

    #[error("Subsystem loading error: {0}")]
    SubsystemLoadingError(#[from] SubsystemLoadingError),
}