use std::pin::pin;
use std::sync::Arc;

use futures::{StreamExt, lock::Mutex};

use common::context::RequestContext;
use exo_env::Environment;

use crate::plugin::{
    SubsystemRpcResolver,
    subsystem_rpc_resolver::{SubsystemRpcError, SubsystemRpcResponse},
};

pub struct SystemRpcResolver {
    subsystem_resolvers: Vec<Box<dyn SubsystemRpcResolver + Send + Sync>>,
    #[allow(dead_code)]
    env: Arc<dyn Environment>,
}

impl SystemRpcResolver {
    pub fn new(
        subsystem_resolvers: Vec<Box<dyn SubsystemRpcResolver + Send + Sync>>,
        env: Arc<dyn Environment>,
    ) -> Self {
        Self {
            subsystem_resolvers,
            env,
        }
    }

    pub async fn resolve<'a>(
        &'a self,
        request_method: &str,
        request_params: &Option<serde_json::Value>,
        request_context: &'a RequestContext<'a>,
        // TODO: Untangle this dependency on GraphQL once we have `@inject Exograph` version of RPC.
        // Then ensure that this function signature mirrors that in GraphQL.
        system_resolver: &'a crate::system_resolver::GraphQLSystemResolver,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        let resolver_stream = futures::stream::iter(self.subsystem_resolvers.iter());

        let request_context_mutex = Mutex::new(request_context);

        let stream = resolver_stream.then(|resolver| async {
            let request_context = request_context_mutex.lock().await;
            resolver
                .resolve(
                    request_method,
                    request_params,
                    *request_context,
                    system_resolver,
                )
                .await
        });

        let mut stream = pin!(stream);

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
