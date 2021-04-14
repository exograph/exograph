/// Type such as Int/String/... (primitive) and Concert/Venue/Person etc (composite)
#[derive(Debug, Clone)]
pub struct AstType {
    pub name: String,
    pub kind: AstTypeKind,
    // authorization info etc.
}

#[derive(Debug, Clone)]
pub enum AstTypeKind {
    Primitive,
    Composite {
        fields: Vec<AstField>,
        table_name: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum AstTypeModifier {
    Optional,
    NonNull,
    List,
}

#[derive(Debug, Clone)]
pub struct AstField {
    pub name: String,
    pub type_name: String,
    pub type_modifier: AstTypeModifier,
    pub relation: AstRelation,
}

#[derive(Debug, Clone)]
pub enum AstRelation {
    Pk {
        column_name: Option<String>,
    },
    Scalar {
        column_name: Option<String>,
    },
    ManyToOne {
        column_name: Option<String>,
        other_type_name: String,
        optional: bool,
    },
    OneToMany {
        column_name: Option<String>,
        other_type_name: String,
        optional: bool,
    },
}

impl AstType {
    pub fn field(&self, name: &str) -> Option<&AstField> {
        match &self.kind {
            AstTypeKind::Primitive => None,
            AstTypeKind::Composite { fields, .. } => fields.iter().find(|field| field.name == name),
        }
    }
}

impl AstField {
    pub fn column_name(&self) -> String {
        match &self.relation {
            AstRelation::Pk { column_name }
            | AstRelation::Scalar { column_name }
            | AstRelation::ManyToOne { column_name, .. } => {
                column_name.clone().unwrap_or(self.name.to_string()).clone()
            }
            AstRelation::OneToMany { column_name, .. } => column_name.clone().unwrap(),
        }
    }
}
