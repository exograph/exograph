use crate::{
    request_context::RequestContext, validation::field::ValidatedField, QueryResponse,
    ResolveOperationFn,
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
    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("Subsystem error: {0}")]
    Generic(String),
}

#[derive(Error, Debug)]
pub enum SystemResolutionError {
    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("Subsystem error: {0}")]
    Generic(String),
}
