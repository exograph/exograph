use std::collections::HashMap;

use codemap::{CodeMap, Span};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub trait NodeTypedness
where
    Self::FieldSelection: Serialize + DeserializeOwned + std::fmt::Debug + Clone + PartialEq,
    Self::RelationalOp: Serialize + DeserializeOwned + std::fmt::Debug + Clone + PartialEq,
    Self::Expr: Serialize + DeserializeOwned + std::fmt::Debug + Clone + PartialEq,
    Self::LogicalOp: Serialize + DeserializeOwned + std::fmt::Debug + Clone + PartialEq,
{
    type FieldSelection;
    type RelationalOp;
    type Expr;
    type LogicalOp;
}

#[derive(Serialize, Deserialize)]
pub struct Untyped;
impl NodeTypedness for Untyped {
    type FieldSelection = ();
    type RelationalOp = ();
    type Expr = ();
    type LogicalOp = ();
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstSystem<T: NodeTypedness> {
    pub models: Vec<AstModel<T>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstModel<T: NodeTypedness> {
    pub name: String,
    pub kind: AstModelKind,
    pub fields: Vec<AstField<T>>,
    pub annotations: Vec<AstAnnotation<T>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstModelKind {
    Persistent,
    Context,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstField<T: NodeTypedness> {
    pub name: String,
    pub ast_typ: AstFieldType,
    pub annotations: Vec<AstAnnotation<T>>,
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
pub struct AstAnnotation<T: NodeTypedness> {
    pub name: String,
    pub params: AstAnnotationParams<T>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstAnnotationParams<T: NodeTypedness> {
    /// No parameters (e.g. `@pk`)
    None,
    /// Single parameter (e.g. `@table("concerts"))
    Single(
        AstExpr<T>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    /// Named parameters (e.g. `@range(min=-10, max=10)`)
    Map(
        HashMap<String, AstExpr<T>>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        Vec<(String, Span)>, // store as Vec to check for duplicates later on
    ),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstExpr<T: NodeTypedness> {
    FieldSelection(FieldSelection<T>),
    LogicalOp(LogicalOp<T>),
    RelationalOp(RelationalOp<T>),
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
    NumberLiteral(
        i64,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
}

impl<T: NodeTypedness> AstExpr<T> {
    pub fn span(&self) -> &Span {
        match &self {
            AstExpr::FieldSelection(s) => s.span(),
            AstExpr::StringLiteral(_, s) => s,
            _ => panic!(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum FieldSelection<T: NodeTypedness> {
    Single(Identifier, T::FieldSelection),
    Select(
        Box<FieldSelection<T>>,
        Identifier,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
        T::FieldSelection,
    ),
}

impl<T: NodeTypedness> FieldSelection<T> {
    pub fn span(&self) -> &Span {
        match &self {
            FieldSelection::Select(_, _, s, _) => s,
            _ => panic!(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LogicalOp<T: NodeTypedness> {
    Not(
        Box<AstExpr<T>>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
        T::LogicalOp,
    ),
    And(Box<AstExpr<T>>, Box<AstExpr<T>>, T::LogicalOp),
    Or(Box<AstExpr<T>>, Box<AstExpr<T>>, T::LogicalOp),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RelationalOp<T: NodeTypedness> {
    Eq(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
    Neq(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
    Lt(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
    Lte(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
    Gt(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
    Gte(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
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
