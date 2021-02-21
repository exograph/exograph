#[derive(Debug, Clone)]
pub struct ModelType {
    pub name: String,
    pub kind: ModelTypeKind,
}

#[derive(Debug, Clone)]
pub enum ModelTypeKind {
    Primitive,
    Composite {
        model_fields: Vec<ModelField>,
        table_name: String,
    },
}

#[derive(Debug, Clone)]
pub enum ModelTypeModifier {
    Optional,
    NonNull,
    List,
}

#[derive(Debug, Clone)]
pub struct ModelField {
    pub name: String,
    pub type_name: String,
    pub type_modifier: ModelTypeModifier,
    pub relation: ModelRelation,
}

#[derive(Debug, Clone)]
pub enum ModelRelation {
    Pk { column_name: Option<String> },
    Scalar { column_name: Option<String> },
    ManyToOne { column_name: Option<String> },
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub type_name: String,
    pub type_modifier: ModelTypeModifier,
    pub role: ParameterRole,
}

#[derive(Debug, Clone)]
pub enum ParameterRole {
    Predicate,
    OrderBy,
    Data, // limit/offset
}

#[derive(Debug, Clone)]
pub struct ParameterType {
    pub name: String,
    pub kind: ParameterTypeKind,
}

#[derive(Debug, Clone)]
pub enum ParameterTypeKind {
    Primitive,
    Composite { parameters: Vec<Parameter> },
    Enum { values: Vec<String> },
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: OperationReturnType,
}

#[derive(Debug, Clone)]
pub struct OperationReturnType {
    pub type_name: String,
    pub type_modifier: ModelTypeModifier,
}
