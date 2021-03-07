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
