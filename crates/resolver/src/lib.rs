// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

/// Provides core functionality for handling incoming queries without depending
/// on any specific web framework.
///
/// The `resolve` function is responsible for doing the work, using information
/// extracted from an incoming request, and returning the response as a stream.
mod root_resolver;
mod system_loader;

#[cfg(not(target_family = "wasm"))]
pub mod graphiql;
mod system_router;
pub use root_resolver::{
    create_system_resolver, create_system_resolver_from_system, create_system_resolver_or_exit,
    get_endpoint_http_path, get_playground_http_path, resolve, resolve_in_memory,
};
pub use system_loader::{introspection_mode, IntrospectionMode};
pub use system_router::SystemRouter;
