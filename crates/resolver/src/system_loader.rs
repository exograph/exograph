// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use common::introspection::{introspection_mode, IntrospectionMode};
use common::EnvError;
use core_resolver::context::JwtAuthenticator;
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
use exo_env::Environment;

use tracing::debug;

pub type StaticLoaders = Vec<Box<dyn SubsystemLoader>>;

pub struct SystemLoader;

const EXO_MAX_SELECTION_DEPTH: &str = "EXO_MAX_SELECTION_DEPTH";

impl SystemLoader {
    pub async fn load(
        ir: impl std::io::Read,
        static_loaders: StaticLoaders,
        env: Arc<dyn Environment>,
    ) -> Result<SystemResolver, SystemLoadingError> {
        let serialized_system = SerializableSystem::deserialize_reader(ir)
            .map_err(SystemLoadingError::ModelSerializationError)?;

        Self::load_from_system(serialized_system, static_loaders, env.clone()).await
    }

    pub async fn load_from_system(
        serialized_system: SerializableSystem,
        mut static_loaders: StaticLoaders,
        env: Arc<dyn Environment>,
    ) -> Result<SystemResolver, SystemLoadingError> {
        let SerializableSystem {
            subsystems,
            query_interception_map,
            mutation_interception_map,
            trusted_documents,
        } = serialized_system;

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

        // First build subsystem resolvers

        let mut subsystem_resolvers = vec![];
        for serialized_subsystem in subsystems {
            let mut loader = get_loader(&mut static_loaders, serialized_subsystem.id)?;
            let resolver = loader
                .init(serialized_subsystem.serialized_subsystem, env.as_ref())
                .await
                .map_err(SystemLoadingError::SubsystemLoadingError)?;
            subsystem_resolvers.push(resolver);
        }

        // Then use those resolvers to build the schema
        let schema = Schema::new_from_resolvers(&subsystem_resolvers);

        let subsystem_resolvers =
            Self::with_introspection_resolver(subsystem_resolvers, env.as_ref())?;

        let (normal_query_depth_limit, introspection_query_depth_limit) =
            query_depth_limits(env.as_ref())?;

        let authenticator = JwtAuthenticator::new_from_env(env.as_ref())
            .await
            .map_err(|e| SystemLoadingError::Config(e.to_string()))?;

        Ok(SystemResolver::new(
            subsystem_resolvers,
            query_interception_map,
            mutation_interception_map,
            trusted_documents,
            schema,
            Arc::new(authenticator),
            env,
            normal_query_depth_limit,
            introspection_query_depth_limit,
        ))
    }

    fn with_introspection_resolver(
        mut subsystem_resolvers: Vec<Box<dyn SubsystemResolver + Send + Sync>>,
        env: &dyn Environment,
    ) -> Result<Vec<Box<dyn SubsystemResolver + Send + Sync>>, SystemLoadingError> {
        let schema = || Schema::new_from_resolvers(&subsystem_resolvers);

        Ok(match introspection_mode(env)? {
            IntrospectionMode::Disabled => subsystem_resolvers,
            IntrospectionMode::Enabled => {
                let introspection_resolver = Box::new(IntrospectionResolver::new(schema()));
                subsystem_resolvers.push(introspection_resolver);
                subsystem_resolvers
            }
            IntrospectionMode::Only => {
                // forgo all other resolvers and only use introspection
                let introspection_resolver = Box::new(IntrospectionResolver::new(schema()));
                vec![introspection_resolver]
            }
        })
    }
}

/// Returns the maximum depth of a selection set for normal queries and introspection queries. We
/// hard-code the introspection query depth to 15 to accommodate the query invoked by GraphQL
/// Playground
pub fn query_depth_limits(env: &dyn Environment) -> Result<(usize, usize), SystemLoadingError> {
    const DEFAULT_QUERY_DEPTH: usize = 5;
    const DEFAULT_INTROSPECTION_QUERY_DEPTH: usize = 15;

    let query_depth = match env.get(EXO_MAX_SELECTION_DEPTH) {
        Some(e) => match e.parse::<usize>() {
            Ok(v) => Ok(v),
            Err(_) => Err(SystemLoadingError::Config(format!(
                "{EXO_MAX_SELECTION_DEPTH} env var must be set to a positive integer"
            ))),
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

    #[error("{0}")]
    EnvError(#[from] EnvError),
}
