use serde::{Deserialize, Serialize};

// use exo_sql::PhysicalTable;

// use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresOperation {
    // TODO: Add parameter model
    // pub table_id: SerializableSlabIndex<PhysicalTable>,
}
