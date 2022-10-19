use async_graphql_parser::{
    types::{FieldDefinition, OperationType, TypeDefinition},
    Positioned,
};
use core_plugin::interception::InterceptorIndex;
use core_resolver::{
    introspection::definition::schema::Schema,
    plugin::{SubsystemResolutionError, SubsystemResolver},
    request_context::RequestContext,
    system_resolver::SystemResolver,
    validation::field::ValidatedField,
    InterceptedOperation, QueryResponse, QueryResponseBody,
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

            Some(body.map(|body| QueryResponse {
                body: QueryResponseBody::Json(body),
                headers: vec![],
            }))
        } else {
            None
        }
    }

    async fn invoke_interceptor<'a>(
        &'a self,
        _interceptor_index: InterceptorIndex,
        _proceeding_interceptor: &'a InterceptedOperation<'a>,
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
