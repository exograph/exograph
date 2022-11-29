use std::{
    env::current_exe,
    path::{Path, PathBuf},
};

use crate::core_model::mapped_arena::MappedArena;
use crate::core_model_builder::{
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    plugin::SubsystemBuild,
    typechecker::{annotation::AnnotationSpec, typ::Type},
};
use crate::core_resolver::plugin::SubsystemResolver;
use crate::error::ModelSerializationError;
use thiserror::Error;

use crate::build_info::SubsystemCheckError;

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
    /// is the plugin annotation. Plugin annotations are used to mark what subsystem a service
    /// should be handled by.
    ///
    /// For example, in order to typecheck:
    ///
    /// ```clay
    /// @deno("example.ts")
    /// service ExampleService {
    ///     ...
    /// ```
    ///
    /// [SubsystemBuilder::annotations] should provide:
    ///
    /// ```ignore
    /// fn annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
    ///     vec![
    ///         ("deno", AnnotationSpec {
    ///             targets: &[AnnotationTarget::Service],
    ///             no_params: false,
    ///             single_params: true,
    ///             mapped_params: None,
    ///         })
    ///     ]
    /// }
    /// ```
    ///
    fn annotations(&self) -> Vec<(&'static str, AnnotationSpec)>;

    /// Build a subsystem's model, producing an [Option<SubsystemBuild>].
    ///
    /// - `typechecked_system`: A partially typechecked system. This contains the set of all [Type]s
    ///                         that were successfully parsed from the user's model, ranging from `service`s
    ///                         to composite types like `model`s and `type`s (not to be confused with [Type]s).
    /// - `base_system`: The base model system for Claytip. These are a set of common types that are
    ///                  used by all plugins, like `context`s and primitive types (`Int`, `String`, etc.)
    ///
    /// Return variants:
    ///
    /// - `Ok(Some(SubsystemBuild { .. }))`: The subsystem was built successfully.
    /// - `Ok(None)`: The subsystem was built successfully, but there were no user-declared services.
    /// - `Err(ModelBuildingError { .. })`: The subsystem was not built successfully.
    fn build(
        &self,
        typechecked_system: &MappedArena<Type>,
        base_system: &BaseModelSystem,
    ) -> Result<Option<SubsystemBuild>, ModelBuildingError>;
}

pub trait SubsystemLoader {
    /// Unique string to identify the subsystem by. Should be shared with the corresponding
    /// [SubsystemBuilder].
    fn id(&self) -> &'static str;

    /// Loads and initializes the subsystem, producing a [SubsystemResolver].
    fn init(
        &self,
        serialized_subsystem: Vec<u8>,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError>;
}

#[derive(Error, Debug)]
pub enum SubsystemLoadingError {
    #[error("System serialization error: {0}")]
    ModelSerializationError(#[from] ModelSerializationError),

    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Error, Debug)]
pub enum LibraryLoadingError {
    #[error("Library not found at {0}")]
    LibraryNotFound(PathBuf),

    #[error("Error while loading subsystem library: {0}")]
    LibraryLoadingError(#[from] libloading::Error),

    #[error("Error while loading symbol {0} from library: {0}")]
    SymbolLoadingError(String, libloading::Error),

    #[error("Error while opening subsystem library: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Subsystem library check failed: {0}")]
    CheckFailed(#[from] SubsystemCheckError),
}

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

/// Loads a subsystem builder from a dynamic library.
pub fn load_subsystem_builder(
    library_path: &Path,
) -> Result<Box<dyn SubsystemBuilder>, LibraryLoadingError> {
    load_subsystem_library(library_path, "__claytip_subsystem_builder")
}

/// Loads a subsystem loader from a dynamic library.
pub fn load_subsystem_loader(
    library_name: &str,
) -> Result<Box<dyn SubsystemLoader>, LibraryLoadingError> {
    // search executable directory for library
    // TODO: we should try to load from sources LD_LIBRARY_PATH first
    let mut library_path = current_exe()?;
    library_path.pop();
    library_path.push(libloading::library_filename(library_name));

    if !library_path.exists() {
        return Err(LibraryLoadingError::LibraryNotFound(library_path));
    }

    load_subsystem_library(&library_path, "__claytip_subsystem_loader")
}
