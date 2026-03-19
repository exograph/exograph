// Re-export all types from pg-schema's index_spec
pub use exo_sql_pg_schema::index_spec::*;

// Re-export types that were moved to core
pub use exo_sql_core::index_kind::{HNWSParams, IndexKind};
