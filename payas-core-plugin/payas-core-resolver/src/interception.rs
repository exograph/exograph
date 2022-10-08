use async_graphql_parser::types::OperationType;
use async_recursion::async_recursion;

use payas_core_model::serializable_system::{InterceptionTree, InterceptorIndexWithSubsystemIndex};

use futures::StreamExt;

use super::{request_context::RequestContext, validation::field::ValidatedField, QueryResponse};

use crate::system_resolver::{SystemResolutionError, SystemResolver};

pub struct InterceptedOperation<'a> {
    operation_type: OperationType,
    operation: &'a ValidatedField,
    interception_tree: Option<&'a InterceptionTree>,
    system_resolver: &'a SystemResolver,
}

impl<'a> InterceptedOperation<'a> {
    pub fn new(
        operation_type: OperationType,
        operation: &'a ValidatedField,
        interception_tree: Option<&'a InterceptionTree>,
        system_resolver: &'a SystemResolver,
    ) -> Self {
        Self {
            operation_type,
            operation,
            system_resolver,
            interception_tree,
        }
    }

    #[async_recursion]
    pub async fn resolve(
        &self,
        request_context: &'a RequestContext<'a>,
    ) -> Result<QueryResponse, SystemResolutionError> {
        match self.interception_tree {
            Some(interception_tree) => match interception_tree {
                InterceptionTree::Intercepted {
                    before,
                    core,
                    after,
                } => {
                    self.invoke_non_proceeding_interceptors(&before, request_context)
                        .await?;
                    let response = {
                        let inner_intercepted_operation = InterceptedOperation {
                            operation_type: self.operation_type,
                            operation: self.operation,
                            system_resolver: self.system_resolver,
                            interception_tree: Some(core.as_ref()),
                        };

                        inner_intercepted_operation.resolve(request_context).await?
                    };
                    self.invoke_non_proceeding_interceptors(after, request_context)
                        .await?;

                    Ok(response)
                }
                InterceptionTree::Around { core, interceptor } => {
                    let raw_response = self
                        .invoke_interceptor(interceptor, Some(core.as_ref()), request_context)
                        .await?;

                    Ok(raw_response
                        .ok_or(SystemResolutionError::AroundInterceptorReturnedNoResponse)?)
                }
                InterceptionTree::Plain => self.resolve_operation(request_context).await,
            },
            None => self.resolve_operation(request_context).await,
        }
    }

    async fn resolve_operation<'e>(
        &self,
        request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse, SystemResolutionError> {
        let stream = futures::stream::iter(self.system_resolver.subsystem_resolvers.iter()).then(
            |resolver| async {
                resolver
                    .resolve(
                        self.operation,
                        self.operation_type,
                        request_context,
                        self.system_resolver,
                    )
                    .await
            },
        );

        futures::pin_mut!(stream);

        // Really a find_map(), but StreamExt::find_map() is not available
        while let Some(next_val) = stream.next().await {
            if let Some(val) = next_val {
                // Found a resolver that could return a value (or an error), so we are done resolving
                return val.map_err(|e| e.into());
            }
        }

        Err(SystemResolutionError::NoResolverFound)
    }

    // Useful for before/after interceptors
    async fn invoke_non_proceeding_interceptors(
        &self,
        interceptors: &Vec<InterceptorIndexWithSubsystemIndex>,
        request_context: &'a RequestContext<'a>,
    ) -> Result<(), SystemResolutionError> {
        for interceptor in interceptors {
            self.invoke_interceptor(interceptor, None, request_context)
                .await?;
        }

        Ok(())
    }

    async fn invoke_interceptor(
        &'a self,
        interceptor: &InterceptorIndexWithSubsystemIndex,
        proceeding_interception_tree: Option<&'a InterceptionTree>,
        request_context: &'a RequestContext<'a>,
    ) -> Result<Option<QueryResponse>, SystemResolutionError> {
        let interceptor_subsystem =
            &self.system_resolver.subsystem_resolvers[interceptor.subsystem_index];

        match proceeding_interception_tree {
            Some(proceeding_interception_tree) => interceptor_subsystem
                .invoke_proceeding_interceptor(
                    self.operation,
                    self.operation_type,
                    interceptor.interceptor_index,
                    proceeding_interception_tree,
                    request_context,
                    self.system_resolver,
                ),

            None => interceptor_subsystem.invoke_non_proceeding_interceptor(
                self.operation,
                self.operation_type,
                interceptor.interceptor_index,
                request_context,
                self.system_resolver,
            ),
        }
        .await
        .map_err(|e| e.into())
    }
}
