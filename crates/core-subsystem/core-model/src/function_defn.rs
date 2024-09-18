use crate::{primitive_type::PrimitiveType, types::FieldType};

#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    // pub parameters: Vec<Parameter>,
    pub return_type: FieldType<PrimitiveType>,
}
