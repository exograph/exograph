use async_graphql_parser::types::OperationType;
use async_recursion::async_recursion;

use core_plugin::interception::{InterceptionTree, InterceptorIndexWithSubsystemIndex};

use super::{request_context::RequestContext, validation::field::ValidatedField, QueryResponse};

use crate::system_resolver::{SystemResolutionError, SystemResolver};

pub struct InterceptedOperation<'a> {
    operation_type: OperationType,
    pub operation: &'a ValidatedField,
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
                    self.invoke_non_proceeding_interceptors(before, request_context)
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
                        .system_resolver
                        .invoke_interceptor(
                            interceptor,
                            self.operation_type,
                            self.operation,
                            Some(core.as_ref()),
                            request_context,
                        )
                        .await?;

                    Ok(raw_response
                        .ok_or(SystemResolutionError::AroundInterceptorReturnedNoResponse)?)
                }
                InterceptionTree::Operation => self.resolve_operation(request_context).await,
            },
            None => self.resolve_operation(request_context).await,
        }
    }

    async fn resolve_operation<'e>(
        &self,
        request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse, SystemResolutionError> {
        self.system_resolver
            .resolve_operation(self.operation_type, self.operation, request_context)
            .await
    }

    // Useful for before/after interceptors
    async fn invoke_non_proceeding_interceptors(
        &self,
        interceptors: &Vec<InterceptorIndexWithSubsystemIndex>,
        request_context: &'a RequestContext<'a>,
    ) -> Result<(), SystemResolutionError> {
        for interceptor in interceptors {
            self.system_resolver
                .invoke_interceptor(
                    interceptor,
                    self.operation_type,
                    self.operation,
                    None,
                    request_context,
                )
                .await?;
        }

        Ok(())
    }
}
