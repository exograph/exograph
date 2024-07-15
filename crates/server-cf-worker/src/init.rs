use std::cell::OnceCell;

use exo_env::Environment;
use tracing::level_filters::LevelFilter;
use wasm_bindgen::prelude::*;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

use core_plugin_shared::{
    serializable_system::SerializableSystem, system_serializer::SystemSerializer,
};
use core_resolver::system_resolver::SystemRouter;
use exo_sql::DatabaseClientManager;
use resolver::{create_system_resolver_from_system, SystemRouter};
use worker::console_error;

use crate::{env::WorkerEnvironment, pg::WorkerPostgresConnect};

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    #[cfg(feature = "panic_hook")]
    console_error_panic_hook::set_once();

    Ok(())
}

pub(crate) async fn init(system_bytes: Vec<u8>, env: WorkerEnvironment) -> Result<(), JsValue> {
    setup_tracing(&env);

    RESOLVER.init_resolver(system_bytes, env).await
}

fn setup_tracing(env: &WorkerEnvironment) {
    #[cfg(feature = "panic_hook")]
    console_error_panic_hook::set_once();

    // Set up simple tracing filter.
    // The proper way would be to call `parse_lossy` on the `EnvFilter` builder, but that adds about
    // 300KB to the wasm binary (and makes the total size exceed the recommended 1MB).
    let level: LevelFilter = match env.get("EXO_LOG") {
        Some(level) => match level.to_lowercase().as_str() {
            "trace" => LevelFilter::TRACE,
            "debug" => LevelFilter::DEBUG,
            "info" => LevelFilter::INFO,
            "warn" => LevelFilter::WARN,
            "error" => LevelFilter::ERROR,
            _ => {
                console_error!("Invalid log level: {}. Defaulting to \"warn\"", level);
                LevelFilter::WARN
            }
        },
        None => LevelFilter::WARN,
    };

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .without_time()
        .with_writer(tracing_web::MakeWebConsoleWriter::new())
        .with_filter(level);

    // Use the "try" version to avoid crashing on refreshes
    let _ = tracing_subscriber::registry().with(fmt_layer).try_init();
}

pub(crate) fn get_system_resolver() -> Result<&'static SystemRouter, JsValue> {
    let system_resolver = RESOLVER
        .system_resolver
        .get()
        .ok_or_else(|| JsValue::from_str("Resolver not set"))?;

    Ok(system_resolver)
}

struct SystemResolverHolder {
    system_resolver: OnceCell<SystemRouter>,
}

unsafe impl Send for SystemResolverHolder {}
unsafe impl Sync for SystemResolverHolder {}

static RESOLVER: SystemResolverHolder = SystemResolverHolder {
    system_resolver: OnceCell::new(),
};

impl SystemResolverHolder {
    async fn init_resolver(
        &self,
        system_bytes: Vec<u8>,
        env: WorkerEnvironment,
    ) -> Result<(), JsValue> {
        if self.system_resolver.get().is_some() {
            return Ok(());
        }

        tracing::info!("Creating system resolver");
        let system = SerializableSystem::deserialize(system_bytes)
            .map_err(|e| JsValue::from_str(&format!("Error deserializing system: {:?}", e)))?;

        let client = WorkerPostgresConnect::create_client(&env).await?;

        let resolver = Self::create_resolver(system, client, Box::new(env))
            .await
            .map_err(|e| JsValue::from_str(&format!("Error creating resolver {:?}", e)))?;

        let _ = self
            .system_resolver
            .set(resolver)
            .map_err(|_| JsValue::from_str("Error setting resolver"))?;

        Ok(())
    }

    async fn create_resolver(
        system: SerializableSystem,
        client_manager: DatabaseClientManager,
        env: Box<dyn Environment>,
    ) -> Result<SystemRouter, JsValue> {
        create_system_resolver_from_system(
            system,
            vec![Box::new(postgres_resolver::PostgresSubsystemLoader {
                existing_client: Some(client_manager),
            })],
            env,
        )
        .await
        .map_err(|e| JsValue::from_str(&format!("Error creating system resolver: {:?}", e)))
    }
}
