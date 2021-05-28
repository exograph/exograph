use serde::{Serialize, Deserialize};

/// Type such as Int/String/... (primitive) and Concert/Venue/Person etc (composite)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstSystem {
    pub types: Vec<AstType>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstTypeKind {
    Int {
        autoincrement: bool,
    },
    Other, // For now, catch-all for other primitive types TODO: Add a variant for each supported primitive type
    Composite {
        fields: Vec<AstField>,
        table_name: Option<String>,
    },
}

impl AstTypeKind {
    fn pk_field(&self) -> Option<&AstField> {
        match self {
            AstTypeKind::Composite { fields, .. } => fields
                .iter()
                .find(|field| matches!(&field.relation, AstRelation::Pk { .. })),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstField {
    pub name: String,
    pub typ: AstFieldType,
    pub relation: AstRelation,
    pub column_name: Option<String>, // interpreted as self column, except for OneToMany where it is interpreted as the other table's column
    pub auth: Option<AstExpr>
}

impl AstField {
    pub fn column_name(&self) -> &str {
        self.column_name.as_ref().unwrap_or(&self.name)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstRelation {
    Pk,
    Other { optional: bool },
    // TODO: Add other auto-geneatable columns (Date with now() etc)
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstFieldType {
    Plain(AstType),
    Optional(Box<AstFieldType>),
    List(Box<AstFieldType>),
}

impl AstFieldType {
    pub fn name(&self) -> String {
        match self {
            AstFieldType::Optional(underlying) | AstFieldType::List(underlying) => {
                underlying.name()
            }
            AstFieldType::Plain(base_type) => base_type.name.clone(),
        }
    }

    pub fn optional(&self) -> Self {
        match self {
            AstFieldType::Optional(_) => self.clone(),
            _ => AstFieldType::Optional(Box::new(self.clone())),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstExpr {
    FieldSelection(FieldSelection),
    LogicalOp(LogicalOp),
    RelationalOp(RelationalOp),
    StringLiteral(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LogicalOp {
    Not(Box<AstExpr>),
    And(Box<AstExpr>, Box<AstExpr>),
    Or(Box<AstExpr>, Box<AstExpr>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RelationalOp {
    Eq(Box<AstExpr>, Box<AstExpr>),
    Neq(Box<AstExpr>, Box<AstExpr>),
    Lt(Box<AstExpr>, Box<AstExpr>),
    Lte(Box<AstExpr>, Box<AstExpr>),
    Gt(Box<AstExpr>, Box<AstExpr>),
    Gte(Box<AstExpr>, Box<AstExpr>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum FieldSelection {
    Single(Identifier),
    Select(Box<FieldSelection>, Identifier),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Identifier(pub String);
