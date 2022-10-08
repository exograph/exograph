use crate::{
    request_context::RequestContext, system_resolver::SystemResolver,
    validation::field::ValidatedField, QueryResponse,
};
use async_graphql_parser::{
    types::{FieldDefinition, OperationType, TypeDefinition},
    Positioned,
};
use async_trait::async_trait;
use payas_core_model::serializable_system::{InterceptionTree, InterceptorIndex};
use thiserror::Error;

#[async_trait]
pub trait SubsystemResolver {
    fn id(&self) -> &'static str;

    async fn resolve<'a>(
        &'a self,
        operation: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext,
        system_resolver: &'a SystemResolver,
    ) -> Option<Result<QueryResponse, SubsystemResolutionError>>;

    async fn invoke_non_proceeding_interceptor<'a>(
        &'a self,
        operation: &'a ValidatedField,
        operation_type: OperationType,
        interceptor_index: InterceptorIndex,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError>;

    async fn invoke_proceeding_interceptor<'a>(
        &'a self,
        operation: &'a ValidatedField,
        operation_type: OperationType,
        interceptor_index: InterceptorIndex,
        proceeding_interception_tree: &'a InterceptionTree,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError>;

    fn schema_queries(&self) -> Vec<Positioned<FieldDefinition>>;
    fn schema_mutations(&self) -> Vec<Positioned<FieldDefinition>>;
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

    #[error("{0}")]
    Delegate(#[source] Box<dyn std::error::Error + Send + Sync>),
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
            SubsystemResolutionError::Delegate(error) => error
                .downcast_ref::<SubsystemResolutionError>()
                .and_then(|error| error.user_error_message()),
        }
    }
}
