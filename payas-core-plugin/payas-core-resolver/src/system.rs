use async_graphql_parser::{
    types::{FieldDefinition, TypeDefinition},
    Positioned,
};
use payas_core_model::serializable_system::InterceptionMap;
use thiserror::Error;
use tracing::instrument;

use crate::{
    plugin::SubsystemResolver, request_context::RequestContext,
    validation::validation_error::ValidationError, OperationsPayload, QueryResponse,
};

pub struct SystemResolver {
    pub subsystem_resolvers: Vec<Box<dyn SubsystemResolver + Send + Sync>>,
    pub query_interception_map: InterceptionMap,
    pub mutation_interception_map: InterceptionMap,

    pub allow_introspection: bool,
}

impl SystemResolver {
    /// Resolve the provided top-level operation.
    ///
    /// Goes through the FieldResolver for ValidatedOperation (and thus get free support for `resolve_fields`)
    /// so that we can support fragments in top-level queries in such as:
    ///
    /// ```graphql
    /// {
    ///   ...query_info
    /// }
    ///
    /// fragment query_info on Query {
    ///   __type(name: "Concert") {
    ///     name
    ///   }
    ///
    ///   __schema {
    ///       types {
    ///       name
    ///     }
    ///   }
    /// }
    /// ```
    #[instrument(
        name = "OperationsExecutor::resolve"
        skip_all
        )]
    pub async fn resolve<'e>(
        &'e self,
        operations_payload: OperationsPayload,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Vec<(String, QueryResponse)>, ExecutionError> {
        // let operation = self.validate_operation(operations_payload)?;

        todo!()
    }

    pub fn schema_queries(&self) -> Vec<Positioned<FieldDefinition>> {
        self.subsystem_resolvers
            .iter()
            .fold(vec![], |mut acc, resolver| {
                acc.extend(resolver.schema_queries());
                acc
            })
    }

    pub fn schema_mutations(&self) -> Vec<Positioned<FieldDefinition>> {
        self.subsystem_resolvers
            .iter()
            .fold(vec![], |mut acc, resolver| {
                acc.extend(resolver.schema_mutations());
                acc
            })
    }

    pub fn schema_types(&self) -> Vec<TypeDefinition> {
        self.subsystem_resolvers
            .iter()
            .fold(vec![], |mut acc, resolver| {
                acc.extend(resolver.schema_types());
                acc
            })
    }
}

// Temporary
#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("Execution error: {0}")]
    Generic(String),

    #[error("{0}")]
    Validation(#[from] ValidationError),
}

impl ExecutionError {
    // Message that should be emitted when the error is returned to the user.
    // This should hide any internal details of the error.
    // TODO: Log the details of the error.
    pub fn user_error_message(&self) -> String {
        "todo".to_string()
    }
}
