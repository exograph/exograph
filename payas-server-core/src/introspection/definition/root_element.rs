use async_graphql_parser::types::OperationType;

#[derive(Debug)]
pub struct RootElement<'a> {
    pub operation_type: &'a OperationType,
    pub name: &'a str,
}
