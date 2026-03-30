// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use core_model_builder::{ast::ast_types::AstAccessExpr, typechecker::Typed};

use deno_graphql_model::{
    interceptor::Interceptor,
    operation::{DenoMutation, DenoQuery},
    subsystem::DenoSubsystem,
};
use subsystem_model_builder_util::ModuleSubsystemWithInterceptors;

pub struct ModelDenoSystemWithInterceptors {
    pub underlying: DenoSubsystem,
    pub interceptors: Vec<(AstAccessExpr<Typed>, SerializableSlabIndex<Interceptor>)>,
}

/// Wrap a protocol-agnostic `ModuleSubsystemWithInterceptors` into GraphQL-specific types.
pub fn build_from_module_system(
    module_system: ModuleSubsystemWithInterceptors,
) -> ModelDenoSystemWithInterceptors {
    let underlying_module_system = module_system.underlying;

    let mut queries = MappedArena::default();
    for query in underlying_module_system.queries.values().into_iter() {
        queries.add(&query.name.clone(), DenoQuery(query));
    }

    let mut mutations = MappedArena::default();
    for mutation in underlying_module_system.mutations.values().into_iter() {
        mutations.add(&mutation.name.clone(), DenoMutation(mutation));
    }

    ModelDenoSystemWithInterceptors {
        underlying: DenoSubsystem {
            contexts: underlying_module_system.contexts,
            module_types: underlying_module_system.module_types,
            queries,
            mutations,
            methods: underlying_module_system.methods,
            scripts: underlying_module_system.scripts,
            interceptors: underlying_module_system.interceptors,
        },
        interceptors: module_system.interceptors,
    }
}
