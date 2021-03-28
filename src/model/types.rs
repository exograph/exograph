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
    Pk {
        column_name: Option<String>,
    },
    Scalar {
        column_name: Option<String>,
    },
    ManyToOne {
        column_name: Option<String>,
        type_name: String,
        optional: bool,
    },
    OneToMany {
        column_name: Option<String>,
        type_name: String,
        optional: bool,
    },
}

impl ModelType {
    pub fn model_field(&self, name: &str) -> Option<&ModelField> {
        match &self.kind {
            ModelTypeKind::Primitive => None,
            ModelTypeKind::Composite { model_fields, .. } => model_fields
                .iter()
                .find(|model_field| model_field.name == name),
        }
    }
}

impl ModelField {
    pub fn column_name(&self) -> String {
        match &self.relation {
            ModelRelation::Pk { column_name }
            | ModelRelation::Scalar { column_name }
            | ModelRelation::ManyToOne { column_name, .. } => {
                column_name.clone().unwrap_or(self.name.to_string()).clone()
            }
            ModelRelation::OneToMany { column_name, .. } => column_name.clone().unwrap(),
        }
    }
}
