use async_graphql_parser::{
    types::{FieldDefinition, OperationType, TypeDefinition},
    Positioned,
};
use async_trait::async_trait;

use futures::TryFutureExt;
use payas_core_model::{mapped_arena::SerializableSlabIndex, system_serializer::SystemSerializer};
use payas_core_resolver::{
    plugin::{SubsystemLoader, SubsystemLoadingError, SubsystemResolutionError, SubsystemResolver},
    request_context::RequestContext,
    validation::field::ValidatedField,
    QueryResponse, ResolveOperationFn,
};
use payas_deno::DenoExecutorPool;
use payas_deno_model::{model::ModelDenoSystem, service::ServiceMethod};

use crate::{ClayDenoExecutorPool, DenoExecutionError, DenoOperation, DenoSystemContext};

pub struct DenoSubsystemLoader {}

impl SubsystemLoader for DenoSubsystemLoader {
    fn id(&self) -> &'static str {
        "deno"
    }

    fn init<'a>(
        &self,
        serialized_subsystem: Vec<u8>,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError> {
        let subsystem = ModelDenoSystem::deserialize(serialized_subsystem)?;

        let executor = DenoExecutorPool::new_from_config(crate::clay_config());

        Ok(Box::new(DenoSubsystemResolver {
            subsystem,
            executor,
        }))
    }
}

pub struct DenoSubsystemResolver {
    pub subsystem: ModelDenoSystem,
    pub executor: ClayDenoExecutorPool,
}

#[async_trait]
impl SubsystemResolver for DenoSubsystemResolver {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext<'a>,
        resolve_operation_fn: ResolveOperationFn<'a>,
    ) -> Option<Result<QueryResponse, SubsystemResolutionError>> {
        let operation_name = &field.name;

        let deno_system_context = DenoSystemContext {
            system: &self.subsystem,
            deno_execution_pool: &self.executor,
            resolve_operation_fn,
        };

        let operation = match operation_type {
            OperationType::Query => {
                let query = self.subsystem.queries.get_by_key(operation_name);
                query.map(|query| {
                    create_deno_operation(&self.subsystem, &query.method_id, field, request_context)
                })
            }
            OperationType::Mutation => {
                let mutation = self.subsystem.mutations.get_by_key(operation_name);
                mutation.map(|mutation| {
                    create_deno_operation(
                        &self.subsystem,
                        &mutation.method_id,
                        field,
                        request_context,
                    )
                })
            }
            OperationType::Subscription => Some(Err(DenoExecutionError::Generic(
                "Subscriptions are not supported".to_string(),
            ))),
        };

        match operation {
            Some(Ok(operation)) => Some(
                operation
                    .execute(&deno_system_context)
                    .map_err(|e| e.into())
                    .await,
            ),
            Some(Err(e)) => Some(Err(e.into())),
            None => None,
        }
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

pub(crate) fn create_deno_operation<'a>(
    system: &'a ModelDenoSystem,
    method_id: &Option<SerializableSlabIndex<ServiceMethod>>,
    field: &'a ValidatedField,
    request_context: &'a RequestContext<'a>,
) -> Result<DenoOperation<'a>, DenoExecutionError> {
    // TODO: Remove unwrap() by changing the type of method_id
    let method = &system.methods[method_id.unwrap()];

    Ok(DenoOperation {
        method,
        field,
        request_context,
    })
}

impl From<DenoExecutionError> for SubsystemResolutionError {
    fn from(e: DenoExecutionError) -> Self {
        match e {
            DenoExecutionError::Authorization => SubsystemResolutionError::Authorization,
            _ => SubsystemResolutionError::UserDisplayError(e.user_error_message()),
        }
    }
}
