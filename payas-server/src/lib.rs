use anyhow::Result;
pub use error::ExecutionError;
pub use execution::query_context::QueryResponse;
use execution::query_executor::QueryExecutor;
use introspection::schema::Schema;
use payas_deno::DenoExecutor;
use payas_model::model::system::ModelSystem;
use payas_sql::{Database, DatabaseExecutor};
use serde_json::{Map, Value};

mod data;
mod error;
mod execution;
mod introspection;

/// Opaque type encapsulating the information required by the `resolve`
/// function.
///
/// A server implmentation should call `create_system_info` and store the
/// returned value, passing a reference to it each time it calls `resolve`.
///
/// For example, in actix, this should be added to the server using `app_data`.
pub struct SystemInfo(ModelSystem, Schema, Database, DenoExecutor);

/// Creates the data required by the server endpoint.
pub fn create_system_info(system: ModelSystem, database: Database) -> SystemInfo {
    let schema = Schema::new(&system);
    let deno_executor = DenoExecutor::default();
    SystemInfo(system, schema, database, deno_executor)
}

pub async fn resolve(
    system_info: &SystemInfo,
    operation_name: Option<&str>,
    query_str: &str,
    variables: Option<&Map<String, Value>>,
    claims: Option<Value>,
) -> Result<Vec<(String, execution::query_context::QueryResponse)>> {
    let SystemInfo(system, schema, database, deno_execution) = system_info;

    let database_executor = DatabaseExecutor { database };
    let executor = QueryExecutor {
        system,
        schema,
        database_executor: &database_executor,
        deno_execution,
    };

    executor
        .execute(operation_name, query_str, variables, claims)
        .await
}
