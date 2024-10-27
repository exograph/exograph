// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{env, process::exit, sync::Arc};

use common::logging_tracing;
use core_plugin_interface::interface::SubsystemLoader;

use exo_env::SystemEnvironment;
use router::system_router::{create_system_router_from_file, SystemRouter};

/// Initialize the server by:
/// - Initializing tracing
/// - Creating the system resolver (and return it)
///
/// The `[SystemResolver]` uses static resolvers for subsystems if the corresponding features
/// ("static-<subsystem>-resolver") are enabled. Note that these feature flags also control if the
/// corresponding libraries are statically linked it.
///
/// # Exit codes
/// - 1 - If the exo_ir file doesn't exist or can't be loaded.
pub async fn init() -> SystemRouter {
    logging_tracing::init();

    let exo_ir_file = get_exo_ir_file_name();

    match create_system_router_from_file(
        &exo_ir_file,
        create_static_loaders(),
        Arc::new(SystemEnvironment),
    )
    .await
    {
        Ok(system_router) => system_router,
        Err(error) => {
            println!("{error}");
            exit(1);
        }
    }
}

pub fn create_static_loaders() -> Vec<Box<dyn SubsystemLoader>> {
    vec![
        #[cfg(feature = "static-postgres-resolver")]
        Box::new(postgres_resolver::PostgresSubsystemLoader {
            existing_client: None,
        }),
        #[cfg(feature = "static-deno-resolver")]
        Box::new(deno_resolver::DenoSubsystemLoader {}),
        #[cfg(feature = "static-wasm-resolver")]
        Box::new(wasm_resolver::WasmSubsystemLoader {}),
    ]
}

// pub fn create_static_rest_loaders() -> Vec<Box<dyn RestSubsystemLoader>> {
//     vec![Box::new(
//         // postgres_rest_builder::PostgresRestSubsystemLoader {},
//     )]
// }

fn get_exo_ir_file_name() -> String {
    if env::args().len() > 1 {
        // $ exo-server <model-file-name> extra-arguments...
        println!("Usage: exo-server");
        exit(1)
    }

    // $ exo-server
    "target/index.exo_ir".to_string()
}
