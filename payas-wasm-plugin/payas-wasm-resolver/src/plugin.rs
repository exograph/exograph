use async_graphql_parser::{
    types::{FieldDefinition, OperationType, TypeDefinition},
    Positioned,
};
use async_trait::async_trait;

use futures::TryFutureExt;
use payas_core_model::{
    mapped_arena::SerializableSlabIndex,
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
use payas_wasm::WasmExecutorPool;
use payas_wasm_model::{model::ModelWasmSystem, service::ServiceMethod};

use crate::{WasmExecutionError, WasmOperation, WasmSystemContext};

pub struct WasmSubsystemLoader {}

impl SubsystemLoader for WasmSubsystemLoader {
    fn id(&self) -> &'static str {
        "wasm"
    }

    fn init<'a>(
        &self,
        serialized_subsystem: Vec<u8>,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError> {
        let subsystem = ModelWasmSystem::deserialize(serialized_subsystem)?;

        let executor = WasmExecutorPool::default();

        Ok(Box::new(WasmSubsystemResolver {
            id: self.id(),
            subsystem,
            executor,
        }))
    }
}

pub struct WasmSubsystemResolver {
    pub id: &'static str,
    pub subsystem: ModelWasmSystem,
    pub executor: WasmExecutorPool,
}

#[async_trait]
impl SubsystemResolver for WasmSubsystemResolver {
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

        let wasm_system_context = WasmSystemContext {
            system: &self.subsystem,
            executor_pool: &self.executor,
            resolve_operation_fn: system_resolver.resolve_operation_fn(),
        };

        let operation = match operation_type {
            OperationType::Query => {
                let query = self.subsystem.queries.get_by_key(operation_name);
                query.map(|query| {
                    create_wasm_operation(&self.subsystem, &query.method_id, field, request_context)
                })
            }
            OperationType::Mutation => {
                let mutation = self.subsystem.mutations.get_by_key(operation_name);
                mutation.map(|mutation| {
                    create_wasm_operation(
                        &self.subsystem,
                        &mutation.method_id,
                        field,
                        request_context,
                    )
                })
            }
            OperationType::Subscription => Some(Err(WasmExecutionError::Generic(
                "Subscriptions are not supported".to_string(),
            ))),
        };

        match operation {
            Some(Ok(operation)) => Some(
                operation
                    .execute(&wasm_system_context)
                    .map_err(|e| e.into())
                    .await,
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

pub(crate) fn create_wasm_operation<'a>(
    system: &'a ModelWasmSystem,
    method_id: &Option<SerializableSlabIndex<ServiceMethod>>,
    field: &'a ValidatedField,
    request_context: &'a RequestContext<'a>,
) -> Result<WasmOperation<'a>, WasmExecutionError> {
    // TODO: Remove unwrap() by changing the type of method_id
    let method = &system.methods[method_id.unwrap()];

    Ok(WasmOperation {
        method,
        field,
        request_context,
    })
}

impl From<WasmExecutionError> for SubsystemResolutionError {
    fn from(e: WasmExecutionError) -> Self {
        match e {
            WasmExecutionError::Authorization => SubsystemResolutionError::Authorization,
            _ => SubsystemResolutionError::UserDisplayError(e.user_error_message()),
        }
    }
}
