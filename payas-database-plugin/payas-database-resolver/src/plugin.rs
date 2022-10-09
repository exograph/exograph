use async_graphql_parser::{
    types::{FieldDefinition, OperationType, TypeDefinition},
    Positioned,
};
use async_trait::async_trait;

use payas_core_model::{
    serializable_system::{InterceptionTree, InterceptorIndex},
    system_serializer::SystemSerializer,
};
use payas_core_resolver::{
    plugin::{SubsystemLoader, SubsystemLoadingError, SubsystemResolutionError, SubsystemResolver},
    request_context::RequestContext,
    system_resolver::SystemResolver,
    validation::field::ValidatedField,
    QueryResponse,
};
use payas_database_model::{
    model::ModelDatabaseSystem,
    operation::{DatabaseMutation, DatabaseQuery},
};
use payas_sql::{AbstractOperation, AbstractPredicate, Database, DatabaseExecutor};

use crate::{
    abstract_operation_resolver::resolve_operation,
    database_execution_error::DatabaseExecutionError, database_mutation::operation,
    database_query::compute_select, database_system_context::DatabaseSystemContext,
};

pub struct DatabaseSubsystemLoader {}

impl SubsystemLoader for DatabaseSubsystemLoader {
    fn id(&self) -> &'static str {
        "database"
    }

    fn init<'a>(
        &self,
        serialized_subsystem: Vec<u8>,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError> {
        let subsystem = ModelDatabaseSystem::deserialize(serialized_subsystem)?;

        let database =
            Database::from_env(None).map_err(|e| SubsystemLoadingError::BoxedError(Box::new(e)))?;
        let executor = DatabaseExecutor { database };

        Ok(Box::new(DatabaseSubsystemResolver {
            id: self.id(),
            subsystem,
            executor,
        }))
    }
}

pub struct DatabaseSubsystemResolver {
    pub id: &'static str,
    pub subsystem: ModelDatabaseSystem,
    pub executor: DatabaseExecutor,
}

#[async_trait]
impl SubsystemResolver for DatabaseSubsystemResolver {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a SystemResolver,
    ) -> Option<Result<QueryResponse, SubsystemResolutionError>> {
        let operation_name = &field.name;

        let database_system_context = DatabaseSystemContext {
            system: &self.subsystem,
            database_executor: &self.executor,
            resolve_operation_fn: system_resolver.resolve_operation_fn(),
        };

        let operation = match operation_type {
            OperationType::Query => {
                let query = self.subsystem.queries.get_by_key(operation_name);

                match query {
                    Some(query) => Some(
                        compute_query_sql_operation(
                            query,
                            field,
                            request_context,
                            &database_system_context,
                        )
                        .await,
                    ),
                    None => {
                        return None;
                    }
                }
            }
            OperationType::Mutation => {
                let mutation = self.subsystem.mutations.get_by_key(operation_name);

                match mutation {
                    Some(mutation) => Some(
                        compute_mutation_sql_operation(
                            mutation,
                            field,
                            request_context,
                            &database_system_context,
                        )
                        .await,
                    ),
                    None => {
                        return None;
                    }
                }
            }
            OperationType::Subscription => Some(Err(DatabaseExecutionError::Generic(
                "Subscriptions are not supported".to_string(),
            ))),
        };

        match operation {
            Some(Ok(operation)) => Some(
                resolve_operation(&operation, database_system_context, request_context)
                    .await
                    .map_err(|e| e.into()),
            ),
            Some(Err(e)) => Some(Err(e.into())),
            None => None,
        }
    }

    async fn invoke_proceeding_interceptor<'a>(
        &'a self,
        _operation: &'a ValidatedField,
        _operation_type: OperationType,
        _interceptor_index: InterceptorIndex,
        _proceeding_interception_tree: &'a InterceptionTree,
        _request_context: &'a RequestContext<'a>,
        _system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        Err(SubsystemResolutionError::NoInterceptorFound)
    }

    async fn invoke_non_proceeding_interceptor<'a>(
        &'a self,
        _operation: &'a ValidatedField,
        _operation_type: OperationType,
        _interceptor_index: InterceptorIndex,
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

async fn compute_query_sql_operation<'a>(
    query: &'a DatabaseQuery,
    field: &'a ValidatedField,
    request_context: &'a RequestContext<'a>,
    database_system_context: &DatabaseSystemContext<'a>,
) -> Result<AbstractOperation<'a>, DatabaseExecutionError> {
    compute_select(
        query,
        field,
        AbstractPredicate::True,
        &database_system_context,
        request_context,
    )
    .await
    .map(AbstractOperation::Select)
}

async fn compute_mutation_sql_operation<'a>(
    mutation: &'a DatabaseMutation,
    field: &'a ValidatedField,
    request_context: &'a RequestContext<'a>,
    database_system_context: &DatabaseSystemContext<'a>,
) -> Result<AbstractOperation<'a>, DatabaseExecutionError> {
    operation(mutation, field, &database_system_context, request_context).await
}

impl From<DatabaseExecutionError> for SubsystemResolutionError {
    fn from(e: DatabaseExecutionError) -> Self {
        match e {
            DatabaseExecutionError::Authorization => SubsystemResolutionError::Authorization,
            _ => SubsystemResolutionError::UserDisplayError(e.user_error_message()),
        }
    }
}
