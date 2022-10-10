use async_graphql_parser::types::OperationType;
use payas_core_resolver::introspection::definition::schema::Schema;

#[derive(Debug)]
pub struct IntrospectionRootElement<'a> {
    pub schema: &'a Schema,
    pub operation_type: &'a OperationType,
    pub name: &'a str,
}
