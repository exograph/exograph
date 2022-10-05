use crate::{
    request_context::RequestContext,
    validation::{field::ValidatedField, validation_error::ValidationError},
    QueryResponse, ResolveOperationFn,
};
use async_graphql_parser::{
    types::{FieldDefinition, OperationType, TypeDefinition},
    Positioned,
};
use async_trait::async_trait;
use payas_core_model::error::ModelSerializationError;
use thiserror::Error;

pub trait SubsystemLoader {
    fn id(&self) -> &'static str;

    // TODO: Should `resolve_operation_fn: ResolveOperationFn,` go here?
    fn init(
        &self,
        serialized_subsystem: Vec<u8>,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError>;
}

#[async_trait]
pub trait SubsystemResolver {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext,
        resolve_operation_fn: ResolveOperationFn<'a>,
    ) -> Option<Result<QueryResponse, SubsystemResolutionError>>;

    fn schema_queries(&self) -> Vec<Positioned<FieldDefinition>>;
    fn schema_mutations(&self) -> Vec<Positioned<FieldDefinition>>;
    fn schema_types(&self) -> Vec<TypeDefinition>;
}

#[derive(Error, Debug)]
pub enum SubsystemLoadingError {
    #[error("System serialization error: {0}")]
    Init(#[from] ModelSerializationError),

    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Error, Debug)]
pub enum SubsystemResolutionError {
    #[error("Invalid field {0} for {1}")]
    InvalidField(String, &'static str), // (field name, container type)

    #[error("Not authorized")]
    Authorization,

    #[error("{0}")]
    UserDisplayError(String), // Error message to be displayed to the user (subsystems should hide internal errors through this)
}

impl SubsystemResolutionError {
    pub fn user_error_message(&self) -> String {
        match self {
            SubsystemResolutionError::InvalidField(field_name, container_type) => {
                format!("Invalid field {} for {}", field_name, container_type)
            }
            SubsystemResolutionError::Authorization => "Not authorized".to_string(),
            SubsystemResolutionError::UserDisplayError(message) => message.to_string(),
        }
    }
}

#[derive(Error, Debug)]
pub enum SystemResolutionError {
    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("{0}")]
    Validation(#[from] ValidationError),

    #[error("{0}")]
    SubsystemResolutionError(#[from] SubsystemResolutionError),

    #[error("Subsystem error: {0}")]
    Generic(String),
}

impl SystemResolutionError {
    // Message that should be emitted when the error is returned to the user.
    // This should hide any internal details of the error.
    // TODO: Log the details of the error.
    pub fn user_error_message(&self) -> String {
        match self {
            SystemResolutionError::BoxedError(_) => todo!(),
            SystemResolutionError::Validation(error) => error.to_string(),
            SystemResolutionError::SubsystemResolutionError(error) => error.user_error_message(),
            SystemResolutionError::Generic(_) => todo!(),
        }
    }
}
