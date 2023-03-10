//! Dynamic loader for the deno-resolver subsystem.
use core_plugin_interface::interface::SubsystemLoader;
use deno_resolver::DenoSubsystemLoader;

core_plugin_interface::export_subsystem_loader!(DenoSubsystemLoader {});
