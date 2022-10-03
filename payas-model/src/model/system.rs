use payas_core_model::serializable_system::InterceptionMap;
use payas_database_model::model::ModelDatabaseSystem;
use payas_deno_model::model::ModelDenoSystem;
use payas_wasm_model::model::ModelWasmSystem;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelSystem {
    pub database_subsystem: ModelDatabaseSystem,
    pub deno_subsystem: ModelDenoSystem,
    pub wasm_subsystem: ModelWasmSystem,
    pub query_interception_map: InterceptionMap,
    pub mutation_interception_map: InterceptionMap,
}
