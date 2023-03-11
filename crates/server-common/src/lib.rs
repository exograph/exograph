use std::{env, process::exit};

use core_plugin_interface::interface::SubsystemLoader;
use core_resolver::system_resolver::SystemResolver;

use resolver::create_system_resolver_or_exit;

mod logging_tracing;

/// Initialize the server by:
/// - Initializing logging and tracing
/// - Creating the system resolver (and return it)
///
/// The `[SystemResolver]` uses static resolvers for Postgres and Deno if the corresponding features
/// ("static-postgres-resolver" and "static-deno-resolver") are enabled. Note that these feature
/// flags also control if the corresponding libraries are statically linked it.
///
/// # Exit codes
/// - 1 - If the claypot file doesn't exist or can't be loaded.
pub fn init() -> SystemResolver {
    logging_tracing::init();

    let claypot_file = get_claypot_file_name();

    let static_loaders: Vec<Box<dyn SubsystemLoader>> = vec![
        #[cfg(feature = "static-postgres-resolver")]
        Box::new(postgres_resolver::PostgresSubsystemLoader {}),
        #[cfg(feature = "static-deno-resolver")]
        Box::new(deno_resolver::DenoSubsystemLoader {}),
    ];

    create_system_resolver_or_exit(&claypot_file, static_loaders)
}

fn get_claypot_file_name() -> String {
    let mut args = env::args().skip(1);

    if args.len() > 1 {
        // $ clay-server <model-file-name> extra-arguments...
        println!("Usage: clay-server <claypot-file>");
        exit(1)
    }

    if args.len() == 0 {
        // $ clay-server
        "index.claypot".to_string()
    } else {
        let file_name = args.next().unwrap();

        if file_name.ends_with(".claypot") {
            // $ clay-server concerts.claypot
            file_name
        } else if file_name.ends_with(".clay") {
            // $ clay-server concerts.clay
            format!("{file_name}pot")
        } else {
            println!("The input file {file_name} doesn't appear to be a claypot. You need build one with the 'clay build <model-file-name>' command.");
            exit(1);
        }
    }
}
