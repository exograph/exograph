// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::cell::RefCell;
use std::sync::Arc;

use common::env_const::{EnvError, EXO_INTROSPECTION};
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
use tracing::debug;

// we spawn many resolvers concurrently in integration tests
thread_local! {
    pub static LOCAL_ALLOW_INTROSPECTION: RefCell<Option<IntrospectionMode>> =  const { RefCell::new(None) };
    pub static LOCAL_ENVIRONMENT: RefCell<Option<std::collections::HashMap<String, String>>> =  const {RefCell::new(None) };
}

pub type StaticLoaders = Vec<Box<dyn SubsystemLoader>>;

pub struct SystemLoader;

const EXO_MAX_SELECTION_DEPTH: &str = "EXO_MAX_SELECTION_DEPTH";

impl SystemLoader {
    pub async fn load(
        read: impl std::io::Read,
        static_loaders: StaticLoaders,
    ) -> Result<SystemResolver, SystemLoadingError> {
        let serialized_system = SerializableSystem::deserialize_reader(read)
            .map_err(SystemLoadingError::ModelSerializationError)?;

        Self::process(serialized_system, static_loaders).await
    }

    pub async fn load_from_bytes(
        bytes: Vec<u8>,
        static_loaders: StaticLoaders,
    ) -> Result<SystemResolver, SystemLoadingError> {
        let serialized_system = SerializableSystem::deserialize(bytes)
            .map_err(SystemLoadingError::ModelSerializationError)?;

        Self::process(serialized_system, static_loaders).await
    }

    async fn process(
        serialized_system: SerializableSystem,
        mut static_loaders: StaticLoaders,
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
                // Otherwise try to load a dynamic loader
                debug!("Using dynamic loader for {}", subsystem_id);
                let subsystem_library_name = format!("{subsystem_id}_resolver_dynamic");

                let loader = core_plugin_interface::interface::load_subsystem_loader(
                    &subsystem_library_name,
                )?;
                Ok(loader)
            }
        }

        // First build subsystem resolvers

        let mut subsystem_resolvers = vec![];
        for serialized_subsystem in subsystems {
            let loader = get_loader(&mut static_loaders, serialized_subsystem.id)?;
            let resolver = loader
                .init(serialized_subsystem.serialized_subsystem)
                .await
                .map_err(SystemLoadingError::SubsystemLoadingError)?;
            subsystem_resolvers.push(resolver);
        }

        // Then use those resolvers to build the schema
        let schema = Schema::new_from_resolvers(&subsystem_resolvers);

        let subsystem_resolvers = Self::with_introspection_resolver(subsystem_resolvers)?;

        let (normal_query_depth_limit, introspection_query_depth_limit) = query_depth_limits()?;

        let authenticator = JwtAuthenticator::new_from_env()
            .await
            .map_err(|e| SystemLoadingError::Config(e.to_string()))?;

        Ok(SystemResolver::new(
            subsystem_resolvers,
            query_interception_map,
            mutation_interception_map,
            trusted_documents,
            schema,
            Arc::new(authenticator),
            LOCAL_ENVIRONMENT.with(|f| {
                f.borrow()
                    .clone()
                    .unwrap_or_else(|| std::env::vars().collect())
            }),
            normal_query_depth_limit,
            introspection_query_depth_limit,
        ))
    }

    fn with_introspection_resolver(
        mut subsystem_resolvers: Vec<Box<dyn SubsystemResolver + Send + Sync>>,
    ) -> Result<Vec<Box<dyn SubsystemResolver + Send + Sync>>, SystemLoadingError> {
        let schema = || Schema::new_from_resolvers(&subsystem_resolvers);

        Ok(match introspection_mode()? {
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntrospectionMode {
    Enabled,  // Introspection queries are allowed (typically dev/yolo mode)
    Disabled, // Introspection queries are not allowed (typically in production)
    Only,     // Only introspection queries are allowed (to support "exo playground")
}

pub fn introspection_mode() -> Result<IntrospectionMode, EnvError> {
    LOCAL_ALLOW_INTROSPECTION.with(|f| {
        f.borrow()
            .map(Ok)
            .unwrap_or_else(|| match std::env::var(EXO_INTROSPECTION).ok() {
                Some(e) => match e.to_lowercase().as_str() {
                    "true" | "enabled" | "1" => Ok(IntrospectionMode::Enabled),
                    "false" | "disabled" => Ok(IntrospectionMode::Disabled),
                    "only" => Ok(IntrospectionMode::Only),
                    _ => Err(EnvError::InvalidEnum {
                        env_key: EXO_INTROSPECTION,
                        env_value: e,
                        message: "Must be set to either true, enabled, 1, false, disabled, or only"
                            .to_string(),
                    }),
                },

                None => Ok(IntrospectionMode::Disabled),
            })
    })
}

/// Returns the maximum depth of a selection set for normal queries and introspection queries. We
/// hard-code the introspection query depth to 15 to accommodate the query invoked by GraphQL
/// Playground
pub fn query_depth_limits() -> Result<(usize, usize), SystemLoadingError> {
    const DEFAULT_QUERY_DEPTH: usize = 5;
    const DEFAULT_INTROSPECTION_QUERY_DEPTH: usize = 15;

    let query_depth = match LOCAL_ENVIRONMENT
        .with(|f| {
            f.borrow()
                .as_ref()
                .and_then(|env| env.get(EXO_MAX_SELECTION_DEPTH).cloned())
        })
        .or_else(|| std::env::var(EXO_MAX_SELECTION_DEPTH).ok())
    {
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
