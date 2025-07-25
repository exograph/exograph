// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_env::Environment;
use std::{env, process::exit, sync::Arc};
use thiserror::Error;

use common::logging_tracing::{self, OtelError};
use core_plugin_interface::interface::SubsystemLoader;

use core_router::SystemLoadingError;
use system_router::{SystemRouter, create_system_router_from_file};

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
pub async fn init(env: Arc<dyn Environment>) -> Result<SystemRouter, ServerInitError> {
    logging_tracing::init().await?;

    let exo_ir_file = get_exo_ir_file_name();

    Ok(create_system_router_from_file(&exo_ir_file, create_static_loaders(), env).await?)
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

fn get_exo_ir_file_name() -> String {
    if env::args().len() > 1 {
        // $ exo-server <model-file-name> extra-arguments...
        println!("Usage: exo-server");
        exit(1)
    }

    // $ exo-server
    "target/index.exo_ir".to_string()
}

#[derive(Error, Debug)]
pub enum ServerInitError {
    #[error(transparent)]
    OtelError(#[from] OtelError),

    #[error(transparent)]
    SystemLoadingError(#[from] SystemLoadingError),
}
