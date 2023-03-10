use core_plugin_interface::interface::SubsystemLoader;
use postgres_resolver::PostgresSubsystemLoader;

core_plugin_interface::export_subsystem_loader!(PostgresSubsystemLoader {});
