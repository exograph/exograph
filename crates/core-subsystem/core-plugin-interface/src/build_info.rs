// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#[cfg(not(target_family = "wasm"))]
use std::ffi::{CStr, c_char};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SubsystemCheckError {
    #[error("Subsystem has an incompatible interface version. Expected: `{0}`, found `{1}`")]
    Incompatible(String, String),

    #[cfg(not(target_family = "wasm"))]
    #[error("Could not load Exograph interface version symbol: {0}")]
    SymbolLoadingError(#[from] libloading::Error),

    #[error("Invalid version provided: {0}")]
    ConversionError(#[from] std::str::Utf8Error),
}

/// Interface version string for Exograph libraries
///
/// The output of this function is used in a rudimentary binary compatibility check. `exo-server`
/// will check the output of this function against the interface version string of any library that
/// it load. Libraries export their version strings through the `__exograph_interface_version`
/// pointer.
///
/// This function should incorporate enough information such that Exograph does not inadvertently
pub fn interface_version() -> String {
    mod built_info {
        include!(concat!(env!("OUT_DIR"), "/built.rs"));
    }

    format!(
        "{}, exograph interface version: {}.{}.x",
        built_info::RUSTC_VERSION,
        built_info::PKG_VERSION_MAJOR,
        built_info::PKG_VERSION_MINOR
    )
}

#[cfg(not(target_family = "wasm"))]
/// Checks the interface version of the library against our version to make sure the
/// library is compatible.
pub(crate) fn check_subsystem_library(
    lib: &libloading::Library,
) -> Result<(), SubsystemCheckError> {
    unsafe {
        // load interface version symbol
        let get_version: libloading::Symbol<unsafe extern "C" fn() -> *const c_char> =
            lib.get(b"__exograph_interface_version")?;

        // call symbol, get library's version
        let library_version = CStr::from_ptr(get_version()).to_str()?;

        // get our version
        let our_version = interface_version();

        // compare
        if library_version == our_version {
            Ok(())
        } else {
            Err(SubsystemCheckError::Incompatible(
                our_version,
                library_version.to_string(),
            ))
        }
    }
}

/// Exports the output of [interface_version] as a symbol.
///
/// Do NOT use this macro explicitly! This macro is automatically invoked by
/// `export_subsystem_builder!(...)` and `export_subsystem_loader!(...)`.
#[macro_export]
macro_rules! __export_build_info {
    () => {
        use core::ffi::c_char;
        use std::ffi::CString;

        #[unsafe(no_mangle)]
        pub extern "C" fn __exograph_interface_version() -> *const c_char {
            let version = core_plugin_interface::build_info::interface_version();
            CString::new(version).unwrap().into_raw()
        }
    };
}
