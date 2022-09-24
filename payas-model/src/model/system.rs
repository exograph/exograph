use payas_database_model::model::ModelDatabaseSystem;
use payas_service_model::model::ModelServiceSystem;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelSystem {
    pub database_subsystem: ModelDatabaseSystem,
    pub service_subsystem: ModelServiceSystem,
}
