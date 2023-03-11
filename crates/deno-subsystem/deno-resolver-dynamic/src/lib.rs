//! Dynamic loader for deno-resolver.
use core_plugin_interface::interface::SubsystemLoader;
use deno_resolver::DenoSubsystemLoader;

// See comments in `postgres-resolver-dynamic/src/lib.rs`.
core_plugin_interface::export_subsystem_loader!(DenoSubsystemLoader {});
