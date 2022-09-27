use std::collections::HashMap;

use payas_core_model::mapped_arena::SerializableSlabIndex;
use payas_database_model::model::ModelDatabaseSystem;
use payas_deno_model::{interceptor::Interceptor, model::ModelServiceSystem};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelSystem {
    pub database_subsystem: ModelDatabaseSystem,
    pub service_subsystem: ModelServiceSystem,
    pub query_interceptors: HashMap<String, Vec<SerializableSlabIndex<Interceptor>>>,
    pub mutation_interceptors: HashMap<String, Vec<SerializableSlabIndex<Interceptor>>>,
}
