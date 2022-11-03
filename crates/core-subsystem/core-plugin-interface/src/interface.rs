use std::env::current_exe;

use core_model::mapped_arena::MappedArena;
use core_model_builder::{
    builder::system_builder::BaseModelSystem, error::ModelBuildingError, plugin::SubsystemBuild,
    typechecker::typ::Type,
};
use core_plugin_shared::error::ModelSerializationError;
use core_resolver::plugin::SubsystemResolver;
use thiserror::Error;

use crate::build_info::SubsystemCheckError;

pub trait SubsystemBuilder {
    fn build(
        &self,
        typechecked_system: &MappedArena<Type>,
        base_system: &BaseModelSystem,
    ) -> Option<Result<SubsystemBuild, ModelBuildingError>>;
}

pub trait SubsystemLoader {
    fn id(&self) -> &'static str;

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
    library_name: &str,
    constructor_symbol_name: &str,
) -> Result<Box<T>, LibraryLoadingError> {
    unsafe {
        // build file path to library
        let mut libpath = current_exe()?;
        libpath.pop();
        libpath.push(format!("lib{}.so", library_name));

        // load the library
        let lib = Box::new(libloading::Library::new(&libpath)?);

        // check the subsystem's build info and make sure it is valid to load
        crate::build_info::check_subsystem_library(&lib)?;

        // get the constructor's pointer
        let constructor: libloading::Symbol<unsafe extern "C" fn() -> *mut T> =
            lib.get(constructor_symbol_name.as_bytes()).map_err(|e| {
                LibraryLoadingError::SymbolLoadingError(constructor_symbol_name.to_string(), e)
            })?;

        let obj_raw = constructor(); // construct the object and get its pointer
        let boxed_obj: Box<T> = Box::from_raw(obj_raw); // construct from struct pointer

        Box::leak(lib); // keep library alive & never drop

        // return object
        Ok(boxed_obj)
    }
}

/// Loads a subsystem builder from a dynamic library.
pub fn load_subsystem_builder(
    library_name: &str,
) -> Result<Box<dyn SubsystemBuilder>, LibraryLoadingError> {
    load_subsystem_library(library_name, "__claytip_subsystem_builder")
}

/// Loads a subsystem loader from a dynamic library.
pub fn load_subsystem_loader(
    library_name: &str,
) -> Result<Box<dyn SubsystemLoader>, LibraryLoadingError> {
    load_subsystem_library(library_name, "__claytip_subsystem_loader")
}
