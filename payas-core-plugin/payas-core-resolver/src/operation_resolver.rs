use async_trait::async_trait;
use futures::StreamExt;

use crate::system::{ExecutionError, SystemResolver};
use crate::validation::field::ValidatedField;
use crate::validation::operation::ValidatedOperation;
use crate::FieldResolver;
use crate::{request_context::RequestContext, QueryResponse};

/// Resolver for the root operation.
///
/// The operation may be a query or a mutation and may be for data or for introspection.
///
#[async_trait]
impl FieldResolver<QueryResponse, ExecutionError> for ValidatedOperation {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_resolver: &'e SystemResolver,
        request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse, ExecutionError> {
        let stream = futures::stream::iter(system_resolver.subsystem_resolvers.iter()).then(
            |resolver| async {
                resolver
                    .resolve(
                        field,
                        self.typ,
                        request_context,
                        system_resolver.resolve_operation_fn(),
                    )
                    .await
            },
        );

        futures::pin_mut!(stream);

        // Really find_map(), but StreamExt::find_map() is not available.
        loop {
            match stream.next().await {
                Some(next_val) => {
                    // Found a resolver that could return a value (or an error).
                    if let Some(val) = next_val {
                        break val.map_err(|e| e.into());
                    }
                }
                None => {
                    // The steam has been exhausted, so return error.
                    break Err(ExecutionError::Generic(
                        "No suitable resolver found".to_string(),
                    ));
                }
            }
        }
    }
}
