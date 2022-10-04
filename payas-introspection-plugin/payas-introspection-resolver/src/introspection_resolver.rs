use async_graphql_parser::{
    types::{FieldDefinition, OperationType, TypeDefinition},
    Positioned,
};
use payas_core_resolver::{
    introspection::definition::{root_element::IntrospectionRootElement, schema::Schema},
    plugin::{SubsystemResolutionError, SubsystemResolver},
    request_context::RequestContext,
    validation::field::ValidatedField,
    QueryResponse, QueryResponseBody, ResolveOperationFn,
};

use crate::field_resolver::FieldResolver;
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
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext,
        _resolve_operation_fn: ResolveOperationFn<'a>,
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
