// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{env, process::exit};

use core_plugin_interface::interface::SubsystemLoader;
use core_resolver::system_resolver::SystemResolver;

use resolver::create_system_resolver_or_exit;

mod logging_tracing;

/// Initialize the server by:
/// - Initializing tracing
/// - Creating the system resolver (and return it)
///
/// The `[SystemResolver]` uses static resolvers for Postgres and Deno if the corresponding features
/// ("static-postgres-resolver" and "static-deno-resolver") are enabled. Note that these feature
/// flags also control if the corresponding libraries are statically linked it.
///
/// # Exit codes
/// - 1 - If the exo_ir file doesn't exist or can't be loaded.
pub fn init() -> SystemResolver {
    logging_tracing::init();

    let exo_ir_file = get_exo_ir_file_name();

    let static_loaders: Vec<Box<dyn SubsystemLoader>> = vec![
        #[cfg(feature = "static-postgres-resolver")]
        Box::new(postgres_resolver::PostgresSubsystemLoader {}),
        #[cfg(feature = "static-deno-resolver")]
        Box::new(deno_resolver::DenoSubsystemLoader {}),
    ];

    create_system_resolver_or_exit(&exo_ir_file, static_loaders)
}

fn get_exo_ir_file_name() -> String {
    let mut args = env::args().skip(1);

    if args.len() > 1 {
        // $ exo-server <model-file-name> extra-arguments...
        println!("Usage: exo-server <exo_ir-file>");
        exit(1)
    }

    if args.len() == 0 {
        // $ exo-server
        "index.exo_ir".to_string()
    } else {
        let file_name = args.next().unwrap();

        if file_name.ends_with(".exo_ir") {
            // $ exo-server concerts.exo_ir
            file_name
        } else if file_name.ends_with(".exo") {
            // $ exo-server concerts.exo
            format!("{file_name}_ir")
        } else {
            println!("The input file {file_name} doesn't appear to be a exo_ir. You need build one with the 'exo build <model-file-name>' command.");
            exit(1);
        }
    }
}
