use std::{cell::OnceCell, collections::HashMap};

use wasm_bindgen::prelude::*;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

use core_plugin_shared::{
    serializable_system::SerializableSystem, system_serializer::SystemSerializer,
};
use core_resolver::{context::LOCAL_JWT_SECRET, system_resolver::SystemResolver};
use exo_sql::DatabaseClientManager;
use resolver::{
    create_system_resolver_from_system, IntrospectionMode, LOCAL_ALLOW_INTROSPECTION,
    LOCAL_ENVIRONMENT,
};

use crate::pg::WorkerPostgresConnect;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    #[cfg(feature = "panic_hook")]
    console_error_panic_hook::set_once();

    Ok(())
}

pub(crate) async fn init(env: worker::Env, system_bytes: Vec<u8>) -> Result<(), JsValue> {
    setup_tracing();

    let postgres_url: String = env
        .secret("EXO_POSTGRES_URL")
        .map(|url_binding| url_binding.to_string())
        .map_err(|e| JsValue::from_str(&format!("Error getting POSTGRES_URL: {:?}", e)))?;
    let jwt_secret: Option<String> = env
        .var("EXO_JWT_SECRET")
        .ok()
        .and_then(|secret_binding| Some(secret_binding.to_string()));

    RESOLVER
        .init_resolver(postgres_url, jwt_secret, system_bytes)
        .await
}

fn setup_tracing() {
    #[cfg(feature = "panic_hook")]
    console_error_panic_hook::set_once();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .without_time()
        .with_writer(tracing_web::MakeWebConsoleWriter::new())
        .with_filter(tracing::level_filters::LevelFilter::DEBUG);

    // Use the "try" version to avoid crashing on refreshes
    let _ = tracing_subscriber::registry().with(fmt_layer).try_init();
}

pub(crate) fn get_system_resolver() -> Result<&'static SystemResolver, JsValue> {
    let system_resolver = RESOLVER
        .system_resolver
        .get()
        .ok_or_else(|| JsValue::from_str("Resolver not set"))?;

    Ok(system_resolver)
}

struct SystemResolverHolder {
    system_resolver: OnceCell<SystemResolver>,
}

unsafe impl Send for SystemResolverHolder {}
unsafe impl Sync for SystemResolverHolder {}

static RESOLVER: SystemResolverHolder = SystemResolverHolder {
    system_resolver: OnceCell::new(),
};

impl SystemResolverHolder {
    async fn init_resolver(
        &self,
        postgres_url: String,
        jwt_secret: Option<String>,
        system_bytes: Vec<u8>,
    ) -> Result<(), JsValue> {
        if self.system_resolver.get().is_some() {
            return Ok(());
        }

        tracing::info!("Creating system resolver");
        let system = SerializableSystem::deserialize(system_bytes)
            .map_err(|e| JsValue::from_str(&format!("Error deserializing system: {:?}", e)))?;

        let client = WorkerPostgresConnect::create_client(&postgres_url).await?;

        let resolver = Self::create_resolver(system, jwt_secret, client)
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
        jwt_secret: Option<String>,
        client_manager: DatabaseClientManager,
    ) -> Result<SystemResolver, JsValue> {
        LOCAL_ALLOW_INTROSPECTION.with(|allow| {
            allow.borrow_mut().replace(IntrospectionMode::Enabled);
        });

        LOCAL_ENVIRONMENT.with(|env| {
            env.borrow_mut().replace(HashMap::new());
        });

        if let Some(jwt_secret) = jwt_secret {
            LOCAL_JWT_SECRET.with(|jwt_secret_ref| {
                jwt_secret_ref.borrow_mut().replace(jwt_secret);
            });
        }

        create_system_resolver_from_system(
            system,
            vec![Box::new(postgres_resolver::PostgresSubsystemLoader {
                existing_client: Some(client_manager),
            })],
        )
        .await
        .map_err(|e| JsValue::from_str(&format!("Error creating system resolver: {:?}", e)))
    }
}
