use async_graphql_parser::{
    types::{FieldDefinition, OperationType, TypeDefinition},
    Positioned,
};
use payas_core_model::serializable_system::{InterceptionTree, InterceptorIndex};
use payas_core_resolver::{
    introspection::definition::schema::Schema,
    plugin::{SubsystemResolutionError, SubsystemResolver},
    request_context::RequestContext,
    system_resolver::SystemResolver,
    validation::field::ValidatedField,
    QueryResponse, QueryResponseBody,
};

use crate::{field_resolver::FieldResolver, root_element::IntrospectionRootElement};
pub struct IntrospectionResolver {
    schema: Schema,
}

impl IntrospectionResolver {
    pub fn new(schema: Schema) -> Self {
        Self { schema }
    }
}

#[async_trait::async_trait]
impl SubsystemResolver for IntrospectionResolver {
    fn id(&self) -> &'static str {
        "introspection"
    }

    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext,
        _system_resolver: &'a SystemResolver,
    ) -> Option<Result<QueryResponse, SubsystemResolutionError>> {
        let name = field.name.as_str();

        if name.starts_with("__") {
            let introspection_root = IntrospectionRootElement {
                schema: &self.schema,
                operation_type: &operation_type,
                name,
            };
            let body = introspection_root
                .resolve_field(field, &self.schema, request_context)
                .await;

            Some(
                body.map(|body| QueryResponse {
                    body: QueryResponseBody::Json(body),
                    headers: vec![],
                })
                .map_err(|e| e.into()),
            )
        } else {
            None
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
        vec![]
    }

    fn schema_mutations(&self) -> Vec<Positioned<FieldDefinition>> {
        vec![]
    }

    fn schema_types(&self) -> Vec<TypeDefinition> {
        vec![]
    }
}
