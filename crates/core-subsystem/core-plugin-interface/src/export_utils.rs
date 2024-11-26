// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

// TODO: deduplicate export_subsystem_builder!(...) and export_subsystem_loader!(...)

/// Exports a subsystem builder as a symbol.
///
/// This should be invoked once for every subsystem builder library to
/// export the subsystem builder.
#[macro_export]
macro_rules! export_subsystem_builder {
    ($builder:expr) => {
        use core::ffi::c_void;
        use core_plugin_interface::__export_build_info;
        __export_build_info!();

        #[no_mangle]
        pub extern "C" fn __exograph_subsystem_builder() -> *mut dyn SubsystemBuilder {
            let builder: Box<dyn SubsystemBuilder> = Box::new($builder);
            unsafe { Box::leak(builder) }
        }
    };
}

/// Exports a subsystem loader as a symbol.
///
/// This should be invoked once for every subsystem loader library to
/// export the subsystem loader.
///
/// Caution: You must not call this macro from a crate with `crate-type` other than `cdynlib`.
/// Otherwise, the symbol will be exported once for each plugin, which will cause a linker error.
/// See postgres-graphql-resolver-dynamic and deno-resolver-dynamic for examples.
#[macro_export]
macro_rules! export_subsystem_loader {
    ($loader:expr) => {
        use core::ffi::c_void;
        use core_plugin_interface::__export_build_info;
        __export_build_info!();

        #[no_mangle]
        pub extern "C" fn __exograph_subsystem_loader() -> *mut dyn SubsystemLoader {
            let loader = Box::new($loader);
            unsafe { Box::leak(loader) }
        }
    };
}
