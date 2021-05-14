/// Type such as Int/String/... (primitive) and Concert/Venue/Person etc (composite)
#[derive(Debug, Clone, PartialEq)]
pub struct AstSystem {
    pub types: Vec<AstType>,
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub enum AstTypeModifier {
    Optional,
    NonNull,
    List,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AstField {
    pub name: String,
    pub type_name: String,
    pub type_modifier: AstTypeModifier,
    pub relation: AstRelation,
    pub column_name: Option<String>, // interpreted as self column, except for OneToMany where it is interpreted as the other table's column
}

#[derive(Debug, Clone, PartialEq)]
pub enum AstRelation {
    Pk { auto_generated: bool },
    Other { optional: bool },
    // TODO: Add other auto-geneatable columns (Date with now() etc)
}
