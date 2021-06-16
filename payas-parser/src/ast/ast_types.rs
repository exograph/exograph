use codemap::{CodeMap, Span};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstSystem {
    pub models: Vec<AstModel>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstModel {
    pub name: String,
    pub kind: AstModelKind,
    pub fields: Vec<AstField>,
    pub annotations: Vec<AstAnnotation>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstModelKind {
    Persistent,
    Context,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstField {
    pub name: String,
    pub typ: AstFieldType,
    pub annotations: Vec<AstAnnotation>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstFieldType {
    Plain(
        String,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    Optional(Box<AstFieldType>),
    List(Box<AstFieldType>),
}

impl AstFieldType {
    pub fn name(&self) -> String {
        match self {
            AstFieldType::Optional(underlying) | AstFieldType::List(underlying) => {
                underlying.name()
            }
            AstFieldType::Plain(base_type, _) => base_type.clone(),
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
    pub params: Vec<AstExpr>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstExpr {
    FieldSelection(FieldSelection),
    LogicalOp(LogicalOp),
    RelationalOp(RelationalOp),
    StringLiteral(
        String,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    BooleanLiteral(
        bool,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
}

impl AstExpr {
    pub fn span(&self) -> &Span {
        match &self {
            AstExpr::FieldSelection(s) => s.span(),
            AstExpr::StringLiteral(_, s) => s,
            _ => panic!(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum FieldSelection {
    Single(Identifier),
    Select(
        Box<FieldSelection>,
        Identifier,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
}

impl FieldSelection {
    pub fn span(&self) -> &Span {
        match &self {
            FieldSelection::Select(_, _, s) => s,
            _ => panic!(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LogicalOp {
    Not(
        Box<AstExpr>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
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
pub struct Identifier(
    pub String,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub Span,
);

fn default_span() -> Span {
    let mut tmp_codemap = CodeMap::new();
    tmp_codemap
        .add_file("".to_string(), "".to_string())
        .span
        .subspan(0, 0)
}
