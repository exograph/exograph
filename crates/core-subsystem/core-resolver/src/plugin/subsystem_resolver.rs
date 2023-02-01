use crate::{
    request_context::RequestContext, system_resolver::SystemResolver,
    validation::field::ValidatedField, InterceptedOperation, QueryResponse,
};
use async_graphql_parser::types::{FieldDefinition, OperationType, TypeDefinition};
use async_trait::async_trait;
use core_plugin_shared::interception::InterceptorIndex;
use thiserror::Error;
use tokio::runtime::Handle;

/// Provides resolution of operations and interceptor methods for a subsystem.
///
/// When using a [SubsystemResolver] that has been loaded as a `cdylib`, it is
/// necessary to pass a [Handle]. This is because each dynamic library loaded
/// has its own thread-local storage (TLS), and does not inherit the Tokio context
/// necessary from the calling code. It is necessary to enter the context explicitly
/// by passing the current handle and entering it, otherwise async operations present in
/// the resolver that need a Tokio context will fail.
///
/// Shim methods has been provided for the asynchronous methods present in this trait. They
/// take an additional argument (a [Handle]), and sets up the necessary context before
/// invoking the actual implementation.
#[async_trait]
pub trait SubsystemResolver: Sync {
    /// The id of the subsystem (for debugging purposes)
    fn id(&self) -> &'static str;

    /// Shim method for `resolve`
    async fn resolve_cdylib<'a>(
        &'a self,
        handle: Handle,
        operation: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext,
        system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        let _guard = handle.enter();
        self.resolve(operation, operation_type, request_context, system_resolver)
            .await
    }

    /// Shim method for `invoke_interceptor`
    async fn invoke_interceptor_cdylib<'a>(
        &'a self,
        handle: Handle,
        interceptor_index: InterceptorIndex,
        intercepted_operation: &'a InterceptedOperation,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        let _guard = handle.enter();
        self.invoke_interceptor(
            interceptor_index,
            intercepted_operation,
            request_context,
            system_resolver,
        )
        .await
    }

    /// Resolve an individual operation
    ///
    /// Returns `None` if the operation is not handled by this subsystem
    async fn resolve<'a>(
        &'a self,
        operation: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext,
        system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError>;

    /// Involves an interceptor
    ///
    /// Returns `None` for `QueryResponse` if the interceptor is of before/after type (which is not
    /// expected to return a value)
    async fn invoke_interceptor<'a>(
        &'a self,
        interceptor_index: InterceptorIndex,
        intercepted_operation: &'a InterceptedOperation,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError>;

    // Support for schema creation (and in turn, validation)

    /// Queries supported by this subsystem
    fn schema_queries(&self) -> Vec<FieldDefinition>;
    /// Mutations supported by this subsystem

    fn schema_mutations(&self) -> Vec<FieldDefinition>;
    /// Types supported by this subsystem. This includes types explicitly defined by user types as
    /// well as types derived from user types (such as for predicates)
    fn schema_types(&self) -> Vec<TypeDefinition>;
}

#[derive(Error, Debug)]
pub enum SubsystemResolutionError {
    #[error("Invalid field {0} for {1}")]
    InvalidField(String, &'static str), // (field name, container type)

    #[error("Not authorized")]
    Authorization,

    #[error("{0}")]
    UserDisplayError(String), // Error message to be displayed to the user (subsystems should hide internal errors through this)

    #[error("No interceptor found")]
    NoInterceptorFound, // Almost certainly a programming error (we asked a wrong subsystem)
}

impl SubsystemResolutionError {
    pub fn user_error_message(&self) -> Option<String> {
        match self {
            SubsystemResolutionError::InvalidField(field_name, container_type) => {
                Some(format!("Invalid field {field_name} for {container_type}"))
            }
            SubsystemResolutionError::Authorization => Some("Not authorized".to_string()),
            SubsystemResolutionError::UserDisplayError(message) => Some(message.to_string()),
            SubsystemResolutionError::NoInterceptorFound => None,
        }
    }
}
