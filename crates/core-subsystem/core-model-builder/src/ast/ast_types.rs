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
        String, // the string representation of the number (later, based on the target type, we will parse it to the appropriate type)
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
    NullLiteral(
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
    ObjectLiteral(
        HashMap<String, AstExpr<T>>,
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        Span,
    ),
}

impl<T: NodeTypedness> AstExpr<T> {
    pub fn span(&self) -> Span {
        match &self {
            AstExpr::FieldSelection(s) => *s.span(),
            AstExpr::StringLiteral(_, s) => *s,
            AstExpr::LogicalOp(l) => match l {
                LogicalOp::Not(_, s, _) => *s,
                LogicalOp::And(_, _, s, _) => *s,
                LogicalOp::Or(_, _, s, _) => *s,
            },
            AstExpr::RelationalOp(r) => r.span(),
            AstExpr::BooleanLiteral(_, s) => *s,
            AstExpr::NumberLiteral(_, s) => *s,
            AstExpr::NullLiteral(s) => *s,
            AstExpr::StringList(_, s) => {
                let mut span = s[0].to_owned();
                for s in s.iter().skip(1) {
                    span = span.merge(*s);
                }
                span
            }
            AstExpr::ObjectLiteral(_, s) => *s,
        }
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
                                AstExpr::StringLiteral(value, _) => {
                                    PrimitiveValue::String(value.clone())
                                }
                                AstExpr::BooleanLiteral(value, _) => {
                                    PrimitiveValue::Boolean(*value)
                                }
                                AstExpr::NumberLiteral(value, _) => {
                                    if value.parse::<i64>().is_ok() {
                                        PrimitiveValue::Number(NumberLiteral::Int(
                                            value.parse::<i64>().unwrap(),
                                        ))
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
        expr: Box<AstExpr<T>>, // expression passed to the function such as "du.userId == AuthContext.id && du.read"
        typ: T::FieldSelection,
    },
    NormalCall {
        #[serde(skip_serializing)]
        #[serde(skip_deserializing)]
        #[serde(default = "default_span")]
        span: Span,
        name: Identifier,        // name of the function such as "contains"
        params: Vec<AstExpr<T>>, // parameters passed to the function such as ("ADMIN")
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
