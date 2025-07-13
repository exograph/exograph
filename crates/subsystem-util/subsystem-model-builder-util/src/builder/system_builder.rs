// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::path::Path;

use async_trait::async_trait;
use core_model::mapped_arena::{MappedArena, SerializableSlab, SerializableSlabIndex};
use core_model_builder::{
    ast::ast_types::{AstExpr, AstModule},
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    typechecker::{Typed, typ::TypecheckedSystem},
};
use subsystem_model_util::{
    interceptor::Interceptor,
    module::{ModuleMethod, Script},
    operation::{ModuleMutation, ModuleQuery},
    subsystem::ModuleSubsystem,
    types::ModuleType,
};

use super::{
    module_builder, resolved_builder,
    type_builder::{self, ResolvedTypeEnv},
};

#[async_trait]
pub trait ScriptProcessor {
    async fn process_script(
        &self,
        module: &AstModule<Typed>,
        base_system: &BaseModelSystem,
        typechecked_system: &TypecheckedSystem,
        path: &Path,
    ) -> Result<(String, Vec<u8>), ModelBuildingError>;
}

#[derive(Debug)]
pub struct SystemContextBuilding {
    pub types: MappedArena<ModuleType>,

    // break this into subsystems
    pub queries: MappedArena<ModuleQuery>,

    pub mutations: MappedArena<ModuleMutation>,
    pub methods: MappedArena<ModuleMethod>,
    pub interceptors: SerializableSlab<Interceptor>, // Don't use MappedArena because we use a composite key (module name + method name) here
    pub scripts: MappedArena<Script>,
}

impl Default for SystemContextBuilding {
    fn default() -> Self {
        Self {
            types: MappedArena::default(),
            queries: MappedArena::default(),
            mutations: MappedArena::default(),
            methods: MappedArena::default(),
            interceptors: SerializableSlab::new(),
            scripts: MappedArena::default(),
        }
    }
}

impl SystemContextBuilding {
    pub fn get_id(&self, name: &str) -> Option<SerializableSlabIndex<ModuleType>> {
        self.types.get_id(name)
    }
}

pub struct ModuleSubsystemWithInterceptors {
    pub underlying: ModuleSubsystem,

    pub interceptors: Vec<(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)>,
}

/// Builds a [ModuleSubsystemWithInterceptors], with a subset of [AstModule]s chosen by closure.
///  
/// `module_selection_closure` - A closure that will return `Some(name)` for each [AstModule] the
///                               subsystem supports, where `name` is the annotation name of the plugin
///                               annotation (e.g. `"deno"` for `@deno`).
/// `process_script` - A closure that will process a script at the provided [`Path`] into a runnable form for usage
///                    during subsystem resolution at runtime.
pub async fn build_with_selection(
    typechecked_system: &TypecheckedSystem,
    base_system: &BaseModelSystem,
    module_selection_closure: impl Fn(&AstModule<Typed>) -> Option<String>,
    script_processor: impl ScriptProcessor,
) -> Result<ModuleSubsystemWithInterceptors, ModelBuildingError> {
    let mut building = SystemContextBuilding::default();
    let resolved_system = resolved_builder::build(
        typechecked_system,
        base_system,
        module_selection_closure,
        script_processor,
    )
    .await?;

    let resolved_env = ResolvedTypeEnv {
        contexts: &base_system.contexts,
        resolved_types: resolved_system.module_types,
        resolved_modules: resolved_system.modules,
        function_definitions: &base_system.function_definitions,
    };

    build_shallow_module(&resolved_env, &mut building);
    build_expanded_module(&resolved_env, &mut building)?;

    let interceptors: Vec<(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)> = resolved_env
        .resolved_modules
        .iter()
        .flat_map(|(_, resolved_module)| {
            resolved_module
                .interceptors
                .iter()
                .map(|resolved_interceptor| {
                    let model_interceptor = building
                        .interceptors
                        .iter()
                        .find_map(|(index, i)| {
                            (i.module_name == resolved_interceptor.module_name
                                && i.method_name == resolved_interceptor.method_name)
                                .then_some(index)
                        })
                        .unwrap();

                    (
                        resolved_interceptor.interceptor_kind.expr().clone(),
                        model_interceptor,
                    )
                })
        })
        .collect();

    Ok(ModuleSubsystemWithInterceptors {
        underlying: ModuleSubsystem {
            module_types: building.types.values(),
            queries: building.queries,
            mutations: building.mutations,
            methods: building.methods.values(),
            scripts: building.scripts.values(),
            contexts: base_system.contexts.clone(),
            interceptors: building.interceptors,
        },
        interceptors,
    })
}

fn build_shallow_module(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    let resolved_module_types = &resolved_env.resolved_types;
    let resolved_modules = &resolved_env.resolved_modules;

    type_builder::build_shallow(resolved_module_types, resolved_env.contexts, building);

    module_builder::build_shallow(resolved_module_types, resolved_modules, building);
}

fn build_expanded_module(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    let resolved_methods = &resolved_env
        .resolved_modules
        .iter()
        .map(|(_, s)| s.methods.iter().collect::<Vec<_>>())
        .collect::<Vec<_>>()
        .concat();

    type_builder::build_module_expanded(resolved_methods, resolved_env, building)?;

    module_builder::build_expanded(building);

    Ok(())
}
