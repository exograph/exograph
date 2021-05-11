/// Type such as Int/String/... (primitive) and Concert/Venue/Person etc (composite)
#[derive(Debug, Clone)]
pub struct AstType {
    pub name: String,
    pub kind: AstTypeKind,
    // authorization info etc.
}

impl AstType {
    pub fn pk_field(&self) -> Option<&AstField> {
        self.kind.pk_field()
    }
}

#[derive(Debug, Clone)]
pub enum AstTypeKind {
    Primitive,
    Composite {
        fields: Vec<AstField>,
        table_name: Option<String>,
    },
}

impl AstTypeKind {
    fn pk_field(&self) -> Option<&AstField> {
        match self {
            AstTypeKind::Primitive => None,
            AstTypeKind::Composite { fields, .. } => fields
                .iter()
                .find(|field| matches!(&field.relation, AstRelation::Pk { .. })),
        }
    }
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
