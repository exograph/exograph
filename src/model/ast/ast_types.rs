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
        other_type_column_name: Option<String>,
        other_type_name: String,
    },
}
