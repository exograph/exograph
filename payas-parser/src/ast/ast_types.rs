use serde::{Serialize, Deserialize};

/// Type such as Int/String/... (primitive) and Concert/Venue/Person etc (composite)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstSystem {
    pub models: Vec<AstModel>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstModel {
    pub name: String,
    pub fields: Vec<AstField>,
    pub annotations: Vec<AstAnnotation>
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstField {
    pub name: String,
    pub typ: AstFieldType,
    pub annotations: Vec<AstAnnotation>
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstFieldType {
    Plain(String),
    Optional(Box<AstFieldType>),
    List(Box<AstFieldType>),
}

impl AstFieldType {
    pub fn name(&self) -> String {
        match self {
            AstFieldType::Optional(underlying) | AstFieldType::List(underlying) => {
                underlying.name()
            }
            AstFieldType::Plain(base_type) => base_type.clone(),
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
pub struct AstAnnotation {
    pub name: String,
    pub params: Vec<AstExpr>
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
