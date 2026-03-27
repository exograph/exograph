// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    collections::HashMap,
    convert::TryFrom,
    fmt::{Display, Formatter},
    path::PathBuf,
};

use codemap::{CodeMap, Span};
use core_model::{
    access::CommonAccessPrimitiveExpression,
    context_type::{ContextFieldType, ContextSelection, ContextSelectionElement, ContextType},
    function_defn::FunctionDefinition,
    mapped_arena::MappedArena,
    primitive_type::{NumberLiteral, PrimitiveValue},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{error::ModelBuildingError, typechecker::Typed};

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
    pub types: Vec<AstModel<T>>,
    pub modules: Vec<AstModule<T>>,
    pub imports: Vec<PathBuf>,
    pub declaration_doc_comments: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstModel<T: NodeTypedness> {
    pub name: String,
    pub kind: AstModelKind,
    pub fields: Vec<AstField<T>>,
    pub fragment_references: Vec<AstFragmentReference<T>>,
    pub annotations: T::Annotations,
    pub doc_comments: Option<String>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

impl<T: NodeTypedness> Display for AstModel<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name.as_str())
    }
}

impl<T: NodeTypedness> Display for AstEnum<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name.as_str())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstFragmentReference<T: NodeTypedness> {
    pub name: String,
    pub typ: T::Type,
    pub doc_comments: Option<String>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

impl<T: NodeTypedness> Display for AstFragmentReference<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name.as_str())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstEnum<T: NodeTypedness> {
    pub name: String,
    pub fields: Vec<AstEnumField<T>>,
    pub doc_comments: Option<String>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstEnumField<T: NodeTypedness> {
    pub name: String,
    pub typ: T::Type,
    pub doc_comments: Option<String>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstModule<T: NodeTypedness> {
    pub name: String,
    pub annotations: T::Annotations,
    pub types: Vec<AstModel<T>>,
    pub enums: Vec<AstEnum<T>>,
    pub methods: Vec<AstMethod<T>>,
    pub interceptors: Vec<AstInterceptor<T>>,
    pub base_exofile: PathBuf, // The exo file in which this module is defined. Used to resolve relative imports and js/ts/wasm sources
    pub doc_comments: Option<String>,
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
    pub doc_comments: Option<String>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
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
    pub doc_comments: Option<String>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "default_span")]
    pub span: Span,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AstModelKind {
    Type,     // a type in a module (with semantics assigned by each module plugin)
    Context,  // defines contextual type some information extracted from the request
    Fragment, // a fragment in a module
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AstField<T: NodeTypedness> {
    pub name: String,
    pub typ: AstFieldType<T>,
    pub annotations: T::Annotations,
    pub default_value: Option<AstFieldDefault<T>>,
    pub doc_comments: Option<String>,
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

/// The value part of a field default: either a literal or a context field reference.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstFieldDefaultValue<T: NodeTypedness> {
    Literal(AstLiteral),
    FieldSelection(FieldSelection<T>),
}

impl<T: NodeTypedness> AstFieldDefaultValue<T> {
    pub fn span(&self) -> Span {
        match self {
            AstFieldDefaultValue::Literal(lit) => lit.span(),
            AstFieldDefaultValue::FieldSelection(sel) => *sel.span(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstFieldDefaultKind<T: NodeTypedness> {
    Value(AstFieldDefaultValue<T>),
    Function(String, Vec<AstLiteral>),
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
        Option<String>, // module name (None implies the current module)
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
            AstFieldType::Plain(_, base_type, _, _, _) => base_type.clone(),
        }
    }

    pub fn span(&self) -> Span {
        match self {
            AstFieldType::Plain(_, _, _, _, span) => *span,
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
        AstAnnotationParam<T>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    /// Named parameters (e.g. `@range(min=-10, max=10)`)
    Map(
        HashMap<String, AstAnnotationParam<T>>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        HashMap<String, Vec<Span>>, // store as Vec to check for duplicates later on
    ),
}

impl AstAnnotationParams<Typed> {
    pub fn as_single(&self) -> &AstAnnotationParam<Typed> {
        match self {
            Self::Single(param, _) => param,
            _ => panic!(),
        }
    }

    pub fn as_map(&self) -> &HashMap<String, AstAnnotationParam<Typed>> {
        match self {
            Self::Map(map, _) => map,
            _ => panic!(),
        }
    }
}

/// Shared literal type used by both access expressions and annotation parameters.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstLiteral {
    String(
        String,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    Boolean(
        bool,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    Number(
        String, // the string representation of the number (later, based on the target type, we will parse it to the appropriate type)
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    Null(
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
}

impl AstLiteral {
    pub fn span(&self) -> Span {
        match self {
            AstLiteral::String(_, s) => *s,
            AstLiteral::Boolean(_, s) => *s,
            AstLiteral::Number(_, s) => *s,
            AstLiteral::Null(s) => *s,
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            AstLiteral::String(s, _) => s.clone(),
            _ => panic!("Expected string literal"),
        }
    }

    pub fn as_int(&self) -> i64 {
        match self {
            AstLiteral::Number(n, _) => n.parse::<i64>().unwrap(),
            _ => panic!("Expected number literal"),
        }
    }

    pub fn as_float(&self) -> f64 {
        match self {
            AstLiteral::Number(n, _) => n.parse::<f64>().unwrap(),
            _ => panic!("Expected number literal"),
        }
    }

    pub fn as_boolean(&self) -> bool {
        match self {
            AstLiteral::Boolean(b, _) => *b,
            _ => panic!("Expected boolean literal"),
        }
    }

    pub fn to_common_access_primitive(&self) -> CommonAccessPrimitiveExpression {
        match self {
            AstLiteral::String(value, _) => {
                CommonAccessPrimitiveExpression::StringLiteral(value.clone())
            }
            AstLiteral::Boolean(value, _) => {
                CommonAccessPrimitiveExpression::BooleanLiteral(*value)
            }
            AstLiteral::Number(value, _) => {
                CommonAccessPrimitiveExpression::NumberLiteral(value.clone())
            }
            AstLiteral::Null(_) => CommonAccessPrimitiveExpression::NullLiteral,
        }
    }
}

/// A projection expression — used in @projection annotation values.
/// Defines how response shapes are built for RPC operations.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstProjectionExpr {
    /// A scalar field: `id`, `name`
    Field(
        String,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    /// Self-projection reference: `/basic`, `/pk`
    SelfProjection(
        String,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    /// Relation projection: `owner/basic`, `questions/pk`
    RelationProjection(
        String,
        String,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    /// A list of projection atoms: `[/basic, venue/nameOnly, venue/locationOnly]`
    List(
        Vec<AstProjectionExpr>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
}

impl AstProjectionExpr {
    pub fn span(&self) -> Span {
        match self {
            AstProjectionExpr::Field(_, s) => *s,
            AstProjectionExpr::SelfProjection(_, s) => *s,
            AstProjectionExpr::RelationProjection(_, _, s) => *s,
            AstProjectionExpr::List(_, s) => *s,
        }
    }
}

/// Access control expressions (used in @access annotations and interceptor expressions).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstAccessExpr<T: NodeTypedness> {
    FieldSelection(FieldSelection<T>),
    LogicalOp(LogicalOp<T>),
    RelationalOp(RelationalOp<T>),
    Literal(AstLiteral),
}

impl<T: NodeTypedness> AstAccessExpr<T> {
    pub fn span(&self) -> Span {
        match &self {
            AstAccessExpr::FieldSelection(s) => *s.span(),
            AstAccessExpr::Literal(lit) => lit.span(),
            AstAccessExpr::LogicalOp(l) => match l {
                LogicalOp::Not(_, s, _) => *s,
                LogicalOp::And(_, _, s, _) => *s,
                LogicalOp::Or(_, _, s, _) => *s,
            },
            AstAccessExpr::RelationalOp(r) => r.span(),
        }
    }
}

/// Single annotation parameter value (used inside AstAnnotationParams).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AstAnnotationParam<T: NodeTypedness> {
    /// A literal value (e.g. `@table("concerts")`, `@range(min=1)`, `@access(true)`)
    Literal(AstLiteral),
    /// A list of strings (e.g. `@unique("a", "b")`)
    StringList(
        Vec<String>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        Vec<Span>,
    ),
    /// An object literal (e.g. `@column(mapping={"zip": "azip"})`)
    ObjectLiteral(
        HashMap<String, AstLiteral>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    /// A full access expression (e.g. `@access(self.id == AuthContext.userId)`)
    AccessExpr(AstAccessExpr<T>),
    /// A projection expression (e.g. `@projection(withOwner = /basic + owner/basic)`)
    Projection(AstProjectionExpr),
}

impl<T: NodeTypedness> AstAnnotationParam<T> {
    pub fn span(&self) -> Span {
        match self {
            AstAnnotationParam::Literal(lit) => lit.span(),
            AstAnnotationParam::StringList(_, spans) => {
                let mut span = spans[0].to_owned();
                for s in spans.iter().skip(1) {
                    span = span.merge(*s);
                }
                span
            }
            AstAnnotationParam::ObjectLiteral(_, s) => *s,
            AstAnnotationParam::AccessExpr(expr) => expr.span(),
            AstAnnotationParam::Projection(proj) => proj.span(),
        }
    }

    /// Convert this annotation parameter to an access expression (borrowing).
    /// Literals are wrapped in `AstAccessExpr::Literal`.
    /// AccessExpr variants are cloned.
    /// StringList, ObjectLiteral, and Projection cannot be converted and will panic.
    pub fn to_access_expr(&self) -> AstAccessExpr<T> {
        match self {
            AstAnnotationParam::Literal(lit) => AstAccessExpr::Literal(lit.clone()),
            AstAnnotationParam::AccessExpr(expr) => expr.clone(),
            AstAnnotationParam::StringList(..)
            | AstAnnotationParam::ObjectLiteral(..)
            | AstAnnotationParam::Projection(..) => {
                panic!("Cannot convert non-expression annotation param to access expression")
            }
        }
    }
}

/// Convenience methods for typed annotation parameters.
impl AstAnnotationParam<Typed> {
    pub fn as_literal(&self) -> &AstLiteral {
        match self {
            AstAnnotationParam::Literal(lit) => lit,
            _ => panic!("Expected literal annotation param"),
        }
    }

    pub fn as_string(&self) -> String {
        self.as_literal().as_string()
    }

    pub fn as_boolean(&self) -> bool {
        self.as_literal().as_boolean()
    }

    pub fn as_int(&self) -> i64 {
        self.as_literal().as_int()
    }

    pub fn as_float(&self) -> f64 {
        self.as_literal().as_float()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum FieldSelection<T: NodeTypedness> {
    Single(FieldSelectionElement<T>, T::FieldSelection),
    Select(
        Box<FieldSelection<T>>, // prefix, for example, `self` or `self.documentUsers`
        FieldSelectionElement<T>, // suffix, for example, `documentUsers.exists(...)` or `exists(...)`
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
        T::FieldSelection,
    ),
}

impl FieldSelection<Typed> {
    pub fn path(&self) -> Vec<FieldSelectionElement<Typed>> {
        fn flatten(selection: &FieldSelection<Typed>, acc: &mut Vec<FieldSelectionElement<Typed>>) {
            match selection {
                FieldSelection::Single(elem, _) => acc.push(elem.clone()),
                FieldSelection::Select(path, elem, _, _) => {
                    flatten(path, acc);
                    acc.push(elem.clone());
                }
            }
        }

        let mut acc = vec![];
        flatten(self, &mut acc);
        acc
    }

    pub fn get_context<'a>(
        &self,
        contexts: &'a MappedArena<ContextType>,
        function_definitions: &'a MappedArena<FunctionDefinition>,
    ) -> Result<(ContextSelection, &'a ContextFieldType), ModelBuildingError> {
        let path_elements = self.path();

        if path_elements.len() < 2 {
            Err(ModelBuildingError::Generic(
                "Context path must have at least 2 elements".to_string(),
            ))
        } else {
            let (head, tail) = path_elements.split_first().unwrap();

            let context_type_name = match head {
                FieldSelectionElement::Identifier(name, _, _) => name,
                _ => panic!(),
            };

            let context_type = contexts
                .iter()
                .find(|t| &t.1.name == context_type_name)
                .unwrap()
                .1;

            let path: Vec<_> = tail
                .iter()
                .map(|elem| match elem {
                    FieldSelectionElement::Identifier(name, _, _) => {
                        ContextSelectionElement::Identifier(name.clone())
                    }
                    FieldSelectionElement::HofCall { .. } => {
                        panic!("HofCall not supported in context path")
                    }
                    FieldSelectionElement::NormalCall { name, params, .. } => {
                        let args = params
                            .iter()
                            .map(|param| match param {
                                AstAccessExpr::Literal(AstLiteral::String(value, _)) => {
                                    PrimitiveValue::String(value.clone())
                                }
                                AstAccessExpr::Literal(AstLiteral::Boolean(value, _)) => {
                                    PrimitiveValue::Boolean(*value)
                                }
                                AstAccessExpr::Literal(AstLiteral::Number(value, _)) => {
                                    if let Ok(n) = value.parse::<i64>() {
                                        PrimitiveValue::Number(NumberLiteral::Int(n))
                                    } else {
                                        PrimitiveValue::Number(NumberLiteral::Float(
                                            value.parse::<f64>().unwrap(),
                                        ))
                                    }
                                }
                                _ => panic!("Unsupported parameter type"),
                            })
                            .collect();

                        ContextSelectionElement::NormalCall {
                            function_name: name.0.clone(),
                            args,
                        }
                    }
                })
                .collect();

            let last_element = path.last().unwrap();

            let field_type = match &last_element {
                ContextSelectionElement::Identifier(name) => {
                    &context_type
                        .fields
                        .iter()
                        .find(|field| &field.name == name)
                        .unwrap()
                        .typ
                }
                ContextSelectionElement::NormalCall { function_name, .. } => {
                    &function_definitions
                        .get_by_key(function_name)
                        .unwrap()
                        .return_type
                }
            };

            let (head_path, tail_path) = path.split_first().unwrap();

            let first_element_name = match head_path {
                ContextSelectionElement::Identifier(name, ..) => name.clone(),
                _ => panic!(),
            };

            Ok((
                ContextSelection {
                    context_name: context_type.name.clone(),
                    path: (first_element_name, tail_path.to_vec()),
                },
                field_type,
            ))
        }
    }
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
pub enum FieldSelectionElement<T: NodeTypedness> {
    /// Identifier such as `self` or `documentUsers`
    Identifier(
        String,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
        T::FieldSelection,
    ),
    /// Higher-order function call such as `some(du => du.id == AuthContext.id && du.read)`
    HofCall {
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        span: Span,
        name: Identifier,       // name of the function such as "some" and "every"
        param_name: Identifier, // name of the function parameter such as "du"
        expr: Box<AstAccessExpr<T>>, // expression passed to the function such as "du.userId == AuthContext.id && du.read"
        typ: T::FieldSelection,
    },
    NormalCall {
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        span: Span,
        name: Identifier,              // name of the function such as "contains"
        params: Vec<AstAccessExpr<T>>, // parameters passed to the function such as ("ADMIN")
        typ: T::FieldSelection,
    },
}

impl<T: NodeTypedness> FieldSelectionElement<T> {
    pub fn span(&self) -> &Span {
        match &self {
            FieldSelectionElement::Identifier(_, span, _) => span,
            FieldSelectionElement::HofCall { span, .. } => span,
            FieldSelectionElement::NormalCall { span, .. } => span,
        }
    }

    pub fn typ(&self) -> &T::FieldSelection {
        match &self {
            FieldSelectionElement::Identifier(.., typ) => typ,
            FieldSelectionElement::HofCall { typ, .. } => typ,
            FieldSelectionElement::NormalCall { typ, .. } => typ,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LogicalOp<T: NodeTypedness> {
    Not(
        Box<AstAccessExpr<T>>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
        T::LogicalOp,
    ),
    And(
        Box<AstAccessExpr<T>>,
        Box<AstAccessExpr<T>>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
        T::LogicalOp,
    ),
    Or(
        Box<AstAccessExpr<T>>,
        Box<AstAccessExpr<T>>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
        T::LogicalOp,
    ),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RelationalOp<T: NodeTypedness> {
    Eq(
        Box<AstAccessExpr<T>>,
        Box<AstAccessExpr<T>>,
        T::RelationalOp,
    ),
    Neq(
        Box<AstAccessExpr<T>>,
        Box<AstAccessExpr<T>>,
        T::RelationalOp,
    ),
    Lt(
        Box<AstAccessExpr<T>>,
        Box<AstAccessExpr<T>>,
        T::RelationalOp,
    ),
    Lte(
        Box<AstAccessExpr<T>>,
        Box<AstAccessExpr<T>>,
        T::RelationalOp,
    ),
    Gt(
        Box<AstAccessExpr<T>>,
        Box<AstAccessExpr<T>>,
        T::RelationalOp,
    ),
    Gte(
        Box<AstAccessExpr<T>>,
        Box<AstAccessExpr<T>>,
        T::RelationalOp,
    ),
    In(
        Box<AstAccessExpr<T>>,
        Box<AstAccessExpr<T>>,
        T::RelationalOp,
    ),
}

impl<T: NodeTypedness> RelationalOp<T> {
    pub fn typ(&self) -> &T::RelationalOp {
        let (_, _, typ) = self.to_tuple();
        typ
    }

    pub fn sides(&self) -> (&AstAccessExpr<T>, &AstAccessExpr<T>) {
        let (l, r, _) = self.to_tuple();
        (l, r)
    }

    fn to_tuple(&self) -> (&AstAccessExpr<T>, &AstAccessExpr<T>, &T::RelationalOp) {
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

    fn span(&self) -> Span {
        let (l, r) = self.sides();
        l.span().merge(r.span())
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

pub fn default_span() -> Span {
    let mut tmp_codemap = CodeMap::new();
    tmp_codemap
        .add_file("".to_string(), "".to_string())
        .span
        .subspan(0, 0)
}
