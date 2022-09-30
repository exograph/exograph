use std::{
    collections::HashMap,
    convert::TryFrom,
    fmt::{Display, Formatter},
    path::PathBuf,
};

use codemap::{CodeMap, Span};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::typechecker::Typed;

pub trait NodeTypedness
where
    Self::FieldSelection: Serialize + DeserializeOwned + std::fmt::Debug + Clone + PartialEq,
    Self::RelationalOp: Serialize + DeserializeOwned + std::fmt::Debug + Clone + PartialEq,
    Self::Expr: Serialize + DeserializeOwned + std::fmt::Debug + Clone + PartialEq,
    Self::LogicalOp: Serialize + DeserializeOwned + std::fmt::Debug + Clone + PartialEq,
    Self::Field: Serialize + DeserializeOwned + std::fmt::Debug + Clone + PartialEq,
    Self::Annotations: Serialize + DeserializeOwned + std::fmt::Debug + Clone + PartialEq,
    Self::Type: Serialize + DeserializeOwned + std::fmt::Debug + Clone + PartialEq,
    Self: Clone,
{
    type FieldSelection;
    type RelationalOp;
    type Expr;
    type LogicalOp;
    type Field;
    type Annotations;
    type Type;
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Untyped;
impl NodeTypedness for Untyped {
    type FieldSelection = ();
    type RelationalOp = ();
    type Expr = ();
    type LogicalOp = ();
    type Field = ();
    type Annotations = Vec<AstAnnotation<Untyped>>;
    type Type = ();
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstSystem<T: NodeTypedness> {
    pub models: Vec<AstModel<T>>,
    pub services: Vec<AstService<T>>,
    pub imports: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstModel<T: NodeTypedness> {
    pub name: String,
    pub kind: AstModelKind,
    pub fields: Vec<AstField<T>>,
    pub annotations: T::Annotations,
}

impl<T: NodeTypedness> Display for AstModel<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name.as_str())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstService<T: NodeTypedness> {
    pub name: String,
    pub models: Vec<AstModel<T>>,
    pub methods: Vec<AstMethod<T>>,
    pub interceptors: Vec<AstInterceptor<T>>,
    pub annotations: T::Annotations,
    pub base_clayfile: PathBuf,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstMethod<T: NodeTypedness> {
    pub name: String,
    pub typ: AstMethodType, // query or mutation?
    pub arguments: Vec<AstArgument<T>>,
    pub return_type: AstFieldType<T>,
    pub is_exported: bool,
    pub annotations: T::Annotations,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AstMethodType {
    Query,
    Mutation,
}

impl TryFrom<&str> for AstMethodType {
    type Error = ();
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "query" => Ok(AstMethodType::Query),
            "mutation" => Ok(AstMethodType::Mutation),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstInterceptor<T: NodeTypedness> {
    pub name: String,
    pub arguments: Vec<AstArgument<T>>,
    pub annotations: T::Annotations,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AstModelKind {
    Persistent,    // a model intended to be persisted inside the database
    Context,       // defines contextual models for authorization
    NonPersistent, // a non-persistent model used as a type for service methods
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstField<T: NodeTypedness> {
    pub name: String,
    pub typ: AstFieldType<T>,
    pub annotations: T::Annotations,
    pub default_value: Option<AstFieldDefault<T>>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstFieldDefault<T: NodeTypedness> {
    pub kind: AstFieldDefaultKind<T>,

    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstFieldDefaultKind<T: NodeTypedness> {
    Value(AstExpr<T>),
    Function(String, Vec<AstExpr<T>>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstArgument<T: NodeTypedness> {
    pub name: String,
    pub typ: AstFieldType<T>,
    pub annotations: T::Annotations,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstFieldType<T: NodeTypedness> {
    Plain(
        String,
        Vec<AstFieldType<T>>, // type parameters (for example, `Concert` for `Set<Concert>`)
        T::Type,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    Optional(Box<AstFieldType<T>>),
}

impl<T: NodeTypedness> AstFieldType<T> {
    pub fn name(&self) -> String {
        match self {
            AstFieldType::Optional(underlying) => underlying.name(),
            AstFieldType::Plain(base_type, _, _, _) => base_type.clone(),
        }
    }

    pub fn span(&self) -> Span {
        match self {
            AstFieldType::Plain(_, _, _, span) => span.clone(),
            AstFieldType::Optional(underlying) => underlying.span(),
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
        HashMap<String, Vec<Span>>, // store as Vec to check for duplicates later on
    ),
}

impl AstAnnotationParams<Typed> {
    pub fn as_single(&self) -> &AstExpr<Typed> {
        match self {
            Self::Single(expr, _) => expr,
            _ => panic!(),
        }
    }

    pub fn as_map(&self) -> &HashMap<String, AstExpr<Typed>> {
        match self {
            Self::Map(map, _) => map,
            _ => panic!(),
        }
    }
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
    StringList(
        Vec<String>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        Vec<Span>,
    ),
}

impl<T: NodeTypedness> AstExpr<T> {
    pub fn span(&self) -> &Span {
        match &self {
            AstExpr::FieldSelection(s) => s.span(),
            AstExpr::StringLiteral(_, s) => s,
            AstExpr::LogicalOp(l) => match l {
                LogicalOp::Not(e, _, _) => e.span(),
                LogicalOp::And(_, _, s, _) => s,
                LogicalOp::Or(_, _, s, _) => s,
            },
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
    And(
        Box<AstExpr<T>>,
        Box<AstExpr<T>>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
        T::LogicalOp,
    ),
    Or(
        Box<AstExpr<T>>,
        Box<AstExpr<T>>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
        T::LogicalOp,
    ),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RelationalOp<T: NodeTypedness> {
    Eq(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
    Neq(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
    Lt(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
    Lte(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
    Gt(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
    Gte(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
    In(Box<AstExpr<T>>, Box<AstExpr<T>>, T::RelationalOp),
}

impl<T: NodeTypedness> RelationalOp<T> {
    pub fn typ(&self) -> &T::RelationalOp {
        let (_, _, typ) = self.to_tuple();
        typ
    }

    pub fn sides(&self) -> (&AstExpr<T>, &AstExpr<T>) {
        let (l, r, _) = self.to_tuple();
        (l, r)
    }

    fn to_tuple(&self) -> (&AstExpr<T>, &AstExpr<T>, &T::RelationalOp) {
        match self {
            RelationalOp::Eq(l, r, typ) => (l, r, typ),
            RelationalOp::Neq(l, r, typ) => (l, r, typ),
            RelationalOp::Lt(l, r, typ) => (l, r, typ),
            RelationalOp::Lte(l, r, typ) => (l, r, typ),
            RelationalOp::Gt(l, r, typ) => (l, r, typ),
            RelationalOp::Gte(l, r, typ) => (l, r, typ),
            RelationalOp::In(l, r, typ) => (l, r, typ),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
