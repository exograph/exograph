use std::collections::HashMap;

use payas_core_model::mapped_arena::SerializableSlabIndex;
use payas_database_model::model::ModelDatabaseSystem;
use payas_deno_model::{interceptor::Interceptor, model::ModelDenoSystem};
use payas_wasm_model::model::ModelWasmSystem;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelSystem {
    pub database_subsystem: ModelDatabaseSystem,
    pub deno_subsystem: ModelDenoSystem,
    pub wasm_subsystem: ModelWasmSystem,
    pub query_interceptors: HashMap<String, Vec<SerializableSlabIndex<Interceptor>>>,
    pub mutation_interceptors: HashMap<String, Vec<SerializableSlabIndex<Interceptor>>>,
}
