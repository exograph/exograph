use crate::{
    request_context::RequestContext, system_resolver::SystemResolver,
    validation::field::ValidatedField, QueryResponse,
};
use async_graphql_parser::{
    types::{FieldDefinition, OperationType, TypeDefinition},
    Positioned,
};
use async_trait::async_trait;
use payas_core_plugin::interception::{InterceptionTree, InterceptorIndex};
use thiserror::Error;

#[async_trait]
pub trait SubsystemResolver {
    /// The id of the subsystem (for debugging purposes)
    fn id(&self) -> &'static str;

    /// Resolve an individual operation
    ///
    /// Returns `None` if the operation is not handled by this subsystem
    async fn resolve<'a>(
        &'a self,
        operation: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext,
        system_resolver: &'a SystemResolver,
    ) -> Option<Result<QueryResponse, SubsystemResolutionError>>;

    /// Involves an interceptor
    ///
    /// Returns `None` for `QueryResponse` if the interceptor is of before/after type (which is not
    /// expected to return a value)
    async fn invoke_proceeding_interceptor<'a>(
        &'a self,
        operation: &'a ValidatedField,
        operation_type: OperationType,
        interceptor_index: InterceptorIndex,
        proceeding_interception_tree: &'a InterceptionTree,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError>;

    /// NOTE: See https://github.com/payalabs/payas/issues/528
    async fn invoke_non_proceeding_interceptor<'a>(
        &'a self,
        operation: &'a ValidatedField,
        operation_type: OperationType,
        interceptor_index: InterceptorIndex,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError>;

    // Support for schema creation (and in turn, validation)

    /// Queries supported by this subsystem
    fn schema_queries(&self) -> Vec<Positioned<FieldDefinition>>;
    /// Mutations supported by this subsystem
    fn schema_mutations(&self) -> Vec<Positioned<FieldDefinition>>;
    /// Types supported by this subsystem. This includes types explicitly defined by user model as
    /// well as types derived from user model (such as for predicates)
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
            SubsystemResolutionError::InvalidField(field_name, container_type) => Some(format!(
                "Invalid field {} for {}",
                field_name, container_type
            )),
            SubsystemResolutionError::Authorization => Some("Not authorized".to_string()),
            SubsystemResolutionError::UserDisplayError(message) => Some(message.to_string()),
            SubsystemResolutionError::NoInterceptorFound => None,
        }
    }
}
