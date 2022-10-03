use async_graphql_parser::types::OperationType;

#[derive(Debug)]
pub struct IntrospectionRootElement<'a> {
    pub operation_type: &'a OperationType,
    pub name: &'a str,
}
