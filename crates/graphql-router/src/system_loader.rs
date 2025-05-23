// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use core_plugin_shared::interception::InterceptionMap;
use core_plugin_shared::trusted_documents::TrustedDocuments;
use core_router::SystemLoadingError;

use core_resolver::plugin::SubsystemGraphQLResolver;
use core_resolver::{
    introspection::definition::schema::Schema, system_resolver::GraphQLSystemResolver,
};
use exo_env::Environment;

pub struct SystemLoader;

const EXO_MAX_SELECTION_DEPTH: &str = "EXO_MAX_SELECTION_DEPTH";

impl SystemLoader {
    pub fn create_system_resolver(
        mut subsystem_resolvers: Vec<Arc<dyn SubsystemGraphQLResolver + Send + Sync>>,
        introspection_resolver: Option<Arc<dyn SubsystemGraphQLResolver + Send + Sync>>,
        query_interception_map: Arc<InterceptionMap>,
        mutation_interception_map: Arc<InterceptionMap>,
        trusted_documents: TrustedDocuments,
        env: Arc<dyn Environment>,
        schema: Arc<Schema>,
    ) -> Result<GraphQLSystemResolver, SystemLoadingError> {
        if let Some(introspection_resolver) = introspection_resolver {
            subsystem_resolvers.push(introspection_resolver);
        }

        let (normal_query_depth_limit, introspection_query_depth_limit) =
            query_depth_limits(env.as_ref())?;

        Ok(GraphQLSystemResolver::new(
            subsystem_resolvers,
            query_interception_map,
            mutation_interception_map,
            trusted_documents,
            schema,
            env,
            normal_query_depth_limit,
            introspection_query_depth_limit,
        ))
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
