// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::path::PathBuf;

use std::sync::Arc;
#[cfg(not(target_family = "wasm"))]
use std::{env::current_exe, path::Path};

use async_trait::async_trait;

use core_model_builder::plugin::{
    BuildMode, CoreSubsystemBuild, RestSubsystemBuild, RpcSubsystemBuild,
};
use core_model_builder::typechecker::typ::TypecheckedSystem;
use core_model_builder::{
    builder::system_builder::BaseModelSystem, error::ModelBuildingError,
    plugin::GraphQLSubsystemBuild, typechecker::annotation::AnnotationSpec,
};
use core_plugin_shared::error::ModelSerializationError;
use core_plugin_shared::serializable_system::SerializableSubsystem;
use core_resolver::plugin::SubsystemGraphQLResolver;
use core_resolver::plugin::{SubsystemRestResolver, SubsystemRpcResolver};
use thiserror::Error;

use crate::build_info::SubsystemCheckError;
use exo_env::Environment;

pub struct SubsystemBuild {
    pub id: &'static str,
    pub graphql: Option<GraphQLSubsystemBuild>,
    pub rest: Option<RestSubsystemBuild>,
    pub rpc: Option<RpcSubsystemBuild>,
    // Common subsystem that is shared by all API-specific subsystems. For example,
    // the Postgres subsystem may use this to keep database (tables, etc.) definitions.
    pub core: CoreSubsystemBuild,
}

#[async_trait]
pub trait SubsystemBuilder {
    /// Unique string to identify the subsystem by. Should be shared with the corresponding
    /// [SubsystemLoader].
    fn id(&self) -> &'static str;

    /// Subsystem-specific annotations to typecheck during the building phase.
    /// Implementations should provide information about the annotations this plugin supports.
    /// The output is a [Vec], consisting of tuples with the annotation name and what
    /// parameters and targets it supports ([AnnotationSpec]).
    ///
    /// One particular annotation that all plugins should declare (if nothing else)
    /// is the plugin annotation. Plugin annotations are used to mark what subsystem a module
    /// should be handled by.
    ///
    /// For example, in order to typecheck:
    ///
    /// ```exo
    /// @deno("example.ts")
    /// module ExampleModule {
    ///     ...
    /// ```
    ///
    /// [SubsystemBuilder::annotations] should provide:
    ///
    /// ```ignore
    /// fn annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
    ///     vec![
    ///         ("deno", AnnotationSpec {
    ///             targets: &[AnnotationTarget::Module],
    ///             no_params: false,
    ///             single_params: true,
    ///             mapped_params: None,
    ///         })
    ///     ]
    /// }
    /// ```
    ///
    fn annotations(&self) -> Vec<(&'static str, AnnotationSpec)>;

    async fn build(
        &self,
        typechecked_system: &TypecheckedSystem,
        base_system: &BaseModelSystem,
        build_mode: BuildMode,
    ) -> Result<Option<SubsystemBuild>, ModelBuildingError>;
}

#[async_trait]
pub trait GraphQLSubsystemBuilder {
    /// Unique string to identify the subsystem by. Should be shared with the corresponding
    /// [SubsystemLoader].
    fn id(&self) -> &'static str;

    /// Build a subsystem's model, producing an [`Option<SubsystemBuild>`].
    ///
    /// - `typechecked_system`: A partially typechecked system. This contains the set of all types
    ///                         that were successfully parsed from the user's model, ranging from `module` types
    ///                         to composite `type`.
    /// - `base_system`: The base model system for Exograph. These are a set of common types that are
    ///                  used by all plugins, like `context`s and primitive types (`Int`, `String`, etc.)
    /// - `check_only`: Only check the subsystem's model, don't build it. Specifically, for Deno kind of subsystem,
    ///                 this will not create generated code based on module definitions.
    ///
    /// Return variants:
    ///
    /// - `Ok(Some(SubsystemBuild { .. }))`: The subsystem was built successfully.
    /// - `Ok(None)`: There were no user-declared modules (no build is required).
    /// - `Err(ModelBuildingError { .. })`: The subsystem was not built successfully.
    async fn build(
        &self,
        typechecked_system: &TypecheckedSystem,
        base_system: &BaseModelSystem,
        build_mode: BuildMode,
    ) -> Result<Option<GraphQLSubsystemBuild>, ModelBuildingError>;
}

pub struct SubsystemResolver {
    pub graphql: Option<Arc<dyn SubsystemGraphQLResolver + Send + Sync>>,
    pub rest: Option<Box<dyn SubsystemRestResolver + Send + Sync>>,
    pub rpc: Option<Box<dyn SubsystemRpcResolver + Send + Sync>>,
}

impl SubsystemResolver {
    pub fn new(
        graphql: Option<Arc<dyn SubsystemGraphQLResolver + Send + Sync>>,
        rest: Option<Box<dyn SubsystemRestResolver + Send + Sync>>,
        rpc: Option<Box<dyn SubsystemRpcResolver + Send + Sync>>,
    ) -> Self {
        Self { graphql, rest, rpc }
    }
}

#[async_trait]
pub trait SubsystemLoader {
    /// Unique string to identify the subsystem by. Should be shared with the corresponding
    /// [SubsystemBuilder].
    fn id(&self) -> &'static str;

    /// Loads and initializes the subsystem, producing a [SubsystemResolver].
    async fn init(
        &mut self,
        serialized_subsystem: SerializableSubsystem,
        env: Arc<dyn Environment>,
    ) -> Result<Box<SubsystemResolver>, SubsystemLoadingError>;
}

#[derive(Error, Debug)]
pub enum SubsystemLoadingError {
    #[error("System serialization error: {0}")]
    ModelSerializationError(#[from] ModelSerializationError),

    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("Configuration error: {0}")]
    Config(String),
}

#[derive(Error, Debug)]
pub enum LibraryLoadingError {
    #[error("Library not found at {0}")]
    LibraryNotFound(PathBuf),

    #[cfg(not(target_family = "wasm"))]
    #[error("Error while loading subsystem library: {0}")]
    LibraryLoadingError(#[from] libloading::Error),

    #[cfg(not(target_family = "wasm"))]
    #[error("Error while loading symbol {0} from library: {0}")]
    SymbolLoadingError(String, libloading::Error),

    #[error("Error while opening subsystem library: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Subsystem library check failed: {0}")]
    CheckFailed(#[from] SubsystemCheckError),
}

#[cfg(not(target_family = "wasm"))]
/// Loads a constructor function from a subsystem library and invokes it.
/// Returns the resultant object.
///
/// * `library_name` - The name of the library to load (platform-independent).
/// * `constructor_symbol_name` - The symbol the constructor function is under in the library.
fn load_subsystem_library<T: ?Sized>(
    library_path: &Path,
    constructor_symbol_name: &str,
) -> Result<Box<T>, LibraryLoadingError> {
    // load the dynamic library
    let lib = Box::new(
        // SAFETY: see documentation for [libloading::Library::new]
        unsafe { libloading::Library::new(library_path.as_os_str())? },
    );

    // check the subsystem's build info and make sure it is valid to load
    crate::build_info::check_subsystem_library(&lib)?;

    // get the constructor's pointer
    // SAFETY: this is safe as long as the constructor function loaded
    //  a. has no arguments and returns a pointer
    //  b. returns a pointer to a boxed instance of T
    // this needs to be manually guaranteed by matching the symbol names and types appropriately
    let boxed_obj: Box<T> = unsafe {
        let constructor: libloading::Symbol<unsafe extern "C" fn() -> *mut T> =
            lib.get(constructor_symbol_name.as_bytes()).map_err(|e| {
                LibraryLoadingError::SymbolLoadingError(constructor_symbol_name.to_string(), e)
            })?;

        let obj_raw = constructor(); // construct the object and get its pointer
        Box::from_raw(obj_raw) // construct from struct pointer
    };

    Box::leak(lib); // keep library alive & never drop

    // return object
    Ok(boxed_obj)
}

#[cfg(not(target_family = "wasm"))]
/// Loads a subsystem builder from a dynamic library.
pub fn load_subsystem_builder(
    library_path: &Path,
) -> Result<Box<dyn SubsystemBuilder + Send + Sync>, LibraryLoadingError> {
    load_subsystem_library(library_path, "__exograph_subsystem_builder")
}

#[cfg(not(target_family = "wasm"))]
/// Loads a subsystem loader from a dynamic library.
pub fn load_subsystem_loader(
    library_name: &str,
) -> Result<Box<dyn SubsystemLoader + Send + Sync>, LibraryLoadingError> {
    // search executable directory for library
    // TODO: we should try to load from sources LD_LIBRARY_PATH first
    let mut library_path = current_exe()?;
    library_path.pop();
    library_path.push(libloading::library_filename(library_name));

    if !library_path.exists() {
        return Err(LibraryLoadingError::LibraryNotFound(library_path));
    }

    load_subsystem_library(&library_path, "__exograph_subsystem_loader")
}
