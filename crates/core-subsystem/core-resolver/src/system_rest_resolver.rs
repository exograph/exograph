use std::sync::Arc;

use futures::{lock::Mutex, StreamExt};

use common::http::{RequestPayload, ResponsePayload};
use exo_env::Environment;

use crate::plugin::{SubsystemResolutionError, SubsystemRestResolver};

pub struct SystemRestResolver {
    subsystem_resolvers: Vec<Box<dyn SubsystemRestResolver + Send + Sync>>,
    #[allow(dead_code)]
    env: Arc<dyn Environment>,
}

impl SystemRestResolver {
    pub fn new(
        subsystem_resolvers: Vec<Box<dyn SubsystemRestResolver + Send + Sync>>,
        env: Arc<dyn Environment>,
    ) -> Self {
        Self {
            subsystem_resolvers,
            env,
        }
    }

    pub async fn resolve(
        &self,
        request: &(dyn RequestPayload + Send + Sync),
    ) -> Result<Option<ResponsePayload>, SubsystemResolutionError> {
        let resolver_stream = futures::stream::iter(self.subsystem_resolvers.iter());

        let request_mutex = Mutex::new(request);

        let stream = resolver_stream.then(|resolver| async {
            let request = request_mutex.lock().await;
            resolver.resolve(*request).await
        });

        futures::pin_mut!(stream);

        // Really a find_map(), but StreamExt::find_map() is not available
        while let Some(next_val) = stream.next().await {
            if let Some(val) = next_val? {
                // Found a resolver that could return a value (or an error), so we are done resolving
                return Ok(Some(val));
            }
        }

        Ok(None)
    }
}
