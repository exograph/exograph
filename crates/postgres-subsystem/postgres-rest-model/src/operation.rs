use serde::{Deserialize, Serialize};

use exo_sql::PhysicalTable;

use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresOperation {
    pub kind: PostgresOperationKind,
    pub table_id: SerializableSlabIndex<PhysicalTable>,
    // TODO: Add parameter model
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PostgresOperationKind {
    Query,
    Mutation,
}
