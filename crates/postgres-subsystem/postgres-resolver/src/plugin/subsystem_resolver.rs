use async_graphql_parser::{
    types::{FieldDefinition, OperationType, TypeDefinition},
    Positioned,
};
use async_trait::async_trait;

use core_plugin_shared::interception::InterceptorIndex;
use core_resolver::{
    plugin::{SubsystemResolutionError, SubsystemResolver},
    request_context::RequestContext,
    system_resolver::SystemResolver,
    validation::field::ValidatedField,
    InterceptedOperation, QueryResponse,
};
use payas_sql::DatabaseExecutor;
use postgres_model::model::ModelPostgresSystem;

use crate::{
    abstract_operation_resolver::resolve_operation, operation_resolver::OperationResolver,
    postgres_execution_error::PostgresExecutionError,
};

pub struct PostgresSubsystemResolver {
    pub id: &'static str,
    pub subsystem: ModelPostgresSystem,
    pub executor: DatabaseExecutor,
}

#[async_trait]
impl SubsystemResolver for PostgresSubsystemResolver {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext<'a>,
        _system_resolver: &'a SystemResolver,
    ) -> Option<Result<QueryResponse, SubsystemResolutionError>> {
        let operation_name = &field.name;

        let operation = match operation_type {
            OperationType::Query => {
                let query = self.subsystem.queries.get_by_key(operation_name);

                match query {
                    Some(query) => {
                        Some(query.resolve(field, request_context, &self.subsystem).await)
                    }
                    None => None,
                }
            }
            OperationType::Mutation => {
                let mutation = self.subsystem.mutations.get_by_key(operation_name);

                match mutation {
                    Some(mutation) => Some(
                        mutation
                            .resolve(field, request_context, &self.subsystem)
                            .await,
                    ),
                    None => None,
                }
            }
            OperationType::Subscription => Some(Err(PostgresExecutionError::Generic(
                "Subscriptions are not supported".to_string(),
            ))),
        };

        match operation {
            Some(Ok(operation)) => Some(
                resolve_operation(&operation, self, request_context)
                    .await
                    .map_err(|e| e.into()),
            ),
            Some(Err(e)) => Some(Err(e.into())),
            None => None,
        }
    }

    async fn invoke_interceptor<'a>(
        &'a self,
        _interceptor_index: InterceptorIndex,
        _intercepted_operation: &'a InterceptedOperation<'a>,
        _request_context: &'a RequestContext<'a>,
        _system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        Err(SubsystemResolutionError::NoInterceptorFound)
    }

    fn schema_queries(&self) -> Vec<Positioned<FieldDefinition>> {
        self.subsystem.schema_queries()
    }

    fn schema_mutations(&self) -> Vec<Positioned<FieldDefinition>> {
        self.subsystem.schema_mutations()
    }

    fn schema_types(&self) -> Vec<TypeDefinition> {
        self.subsystem.schema_types()
    }
}

impl From<PostgresExecutionError> for SubsystemResolutionError {
    fn from(e: PostgresExecutionError) -> Self {
        match e {
            PostgresExecutionError::Authorization => SubsystemResolutionError::Authorization,
            _ => SubsystemResolutionError::UserDisplayError(e.user_error_message()),
        }
    }
}
