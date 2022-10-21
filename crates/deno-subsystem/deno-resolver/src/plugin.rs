use async_graphql_parser::{
    types::{FieldDefinition, OperationType, TypeDefinition},
    Positioned,
};
use async_trait::async_trait;

use core_model::mapped_arena::SerializableSlabIndex;
use core_plugin::{interception::InterceptorIndex, system_serializer::SystemSerializer};
use core_resolver::{
    claytip_execute_query,
    plugin::{SubsystemLoader, SubsystemLoadingError, SubsystemResolutionError, SubsystemResolver},
    request_context::RequestContext,
    system_resolver::SystemResolver,
    validation::field::ValidatedField,
    InterceptedOperation, QueryResponse, QueryResponseBody,
};
use deno_model::{model::ModelDenoSystem, service::ServiceMethod};
use futures::TryFutureExt;
use payas_deno::DenoExecutorPool;

use super::{
    clay_execution::{clay_config, ClaytipMethodResponse, RequestFromDenoMessage},
    claytip_ops::InterceptedOperationInfo,
    deno_execution_error::DenoExecutionError,
    deno_operation::DenoOperation,
};

pub type ClayDenoExecutorPool = DenoExecutorPool<
    Option<InterceptedOperationInfo>,
    RequestFromDenoMessage,
    ClaytipMethodResponse,
>;
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

        let executor = DenoExecutorPool::new_from_config(clay_config());

        Ok(Box::new(DenoSubsystemResolver {
            id: self.id(),
            subsystem,
            executor,
        }))
    }
}

pub struct DenoSubsystemResolver {
    pub id: &'static str,
    pub subsystem: ModelDenoSystem,
    pub executor: ClayDenoExecutorPool,
}

#[async_trait]
impl SubsystemResolver for DenoSubsystemResolver {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn resolve<'a>(
        &'a self,
        operation: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a SystemResolver,
    ) -> Option<Result<QueryResponse, SubsystemResolutionError>> {
        let operation_name = &operation.name;

        let deno_operation = match operation_type {
            OperationType::Query => {
                let query = self.subsystem.queries.get_by_key(operation_name);
                query.map(|query| {
                    create_deno_operation(
                        &self.subsystem,
                        &query.method_id,
                        operation,
                        request_context,
                        self,
                        system_resolver,
                    )
                })
            }
            OperationType::Mutation => {
                let mutation = self.subsystem.mutations.get_by_key(operation_name);
                mutation.map(|mutation| {
                    create_deno_operation(
                        &self.subsystem,
                        &mutation.method_id,
                        operation,
                        request_context,
                        self,
                        system_resolver,
                    )
                })
            }
            OperationType::Subscription => Some(Err(DenoExecutionError::Generic(
                "Subscriptions are not supported".to_string(),
            ))),
        };

        match deno_operation {
            Some(Ok(operation)) => Some(operation.execute().map_err(|e| e.into()).await),
            Some(Err(e)) => Some(Err(e.into())),
            None => None,
        }
    }

    async fn invoke_interceptor<'a>(
        &'a self,
        interceptor_index: InterceptorIndex,
        intercepted_operation: &'a InterceptedOperation<'a>,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        let interceptor =
            &self.subsystem.interceptors[SerializableSlabIndex::from_idx(interceptor_index.0)];

        let claytip_execute_query = claytip_execute_query!(system_resolver, request_context);
        let (result, response) = super::interceptor_execution::execute_interceptor(
            interceptor,
            self,
            request_context,
            &claytip_execute_query,
            intercepted_operation,
        )
        .await?;

        let body = match result {
            serde_json::Value::String(value) => QueryResponseBody::Raw(Some(value)),
            _ => QueryResponseBody::Json(result),
        };

        Ok(Some(QueryResponse {
            body,
            headers: response.map(|r| r.headers).unwrap_or_default(),
        }))
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
    subsystem_resolver: &'a DenoSubsystemResolver,
    system_resolver: &'a SystemResolver,
) -> Result<DenoOperation<'a>, DenoExecutionError> {
    // TODO: Remove unwrap() by changing the type of method_id
    let method = &system.methods[method_id.unwrap()];

    Ok(DenoOperation {
        method,
        field,
        request_context,
        subsystem_resolver,
        system_resolver,
    })
}

impl From<DenoExecutionError> for SubsystemResolutionError {
    fn from(e: DenoExecutionError) -> Self {
        match e {
            DenoExecutionError::Authorization => SubsystemResolutionError::Authorization,
            _ => SubsystemResolutionError::UserDisplayError(
                e.user_error_message()
                    .unwrap_or_else(|| "Internal server error".to_string()),
            ),
        }
    }
}
