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
use core_plugin_shared::interception::InterceptionMap;
use core_plugin_shared::trusted_documents::TrustedDocuments;
use core_resolver::system_resolver::Schemas;
use core_router::SystemLoadingError;
use introspection_resolver::IntrospectionResolver;

use core_resolver::plugin::SubsystemGraphQLResolver;
use core_resolver::{
    introspection::definition::schema::Schema, system_resolver::GraphQLSystemResolver,
};
use exo_env::Environment;

pub struct SystemLoader;

const EXO_MAX_SELECTION_DEPTH: &str = "EXO_MAX_SELECTION_DEPTH";

impl SystemLoader {
    pub async fn create_system_resolver(
        subsystem_resolvers: Vec<Box<dyn SubsystemGraphQLResolver + Send + Sync>>,
        query_interception_map: InterceptionMap,
        mutation_interception_map: InterceptionMap,
        trusted_documents: TrustedDocuments,
        env: Arc<dyn Environment>,
        allow_mutations: bool,
    ) -> Result<GraphQLSystemResolver, SystemLoadingError> {
        let default_schema = Arc::new(Schema::new_from_resolvers(
            &subsystem_resolvers,
            allow_mutations,
        ));
        let queries_only_schema = if allow_mutations {
            Arc::new(Schema::new_from_resolvers(&subsystem_resolvers, false))
        } else {
            // If the default schema is already queries only, we can just use that
            default_schema.clone()
        };

        let schemas = Arc::new(Schemas::new(default_schema, queries_only_schema));

        let subsystem_resolvers =
            Self::with_introspection_resolver(subsystem_resolvers, schemas.clone(), env.as_ref())?;

        let (normal_query_depth_limit, introspection_query_depth_limit) =
            query_depth_limits(env.as_ref())?;

        Ok(GraphQLSystemResolver::new(
            subsystem_resolvers,
            query_interception_map,
            mutation_interception_map,
            trusted_documents,
            schemas,
            env,
            normal_query_depth_limit,
            introspection_query_depth_limit,
        ))
    }

    fn with_introspection_resolver(
        mut subsystem_resolvers: Vec<Box<dyn SubsystemGraphQLResolver + Send + Sync>>,
        schemas: Arc<Schemas>,
        env: &dyn Environment,
    ) -> Result<Vec<Box<dyn SubsystemGraphQLResolver + Send + Sync>>, EnvError> {
        Ok(match introspection_mode(env)? {
            IntrospectionMode::Disabled => subsystem_resolvers,
            IntrospectionMode::Enabled => {
                let introspection_resolver = Box::new(IntrospectionResolver::new(schemas.clone()));
                subsystem_resolvers.push(introspection_resolver);
                subsystem_resolvers
            }
            IntrospectionMode::Only => {
                // forgo all other resolvers and only use introspection
                let introspection_resolver = Box::new(IntrospectionResolver::new(schemas.clone()));
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
