use std::ops::Deref;

use payas_model::{
    model::mapped_arena::MappedArena,
    sql::column::{IntBits, PhysicalColumnType},
};
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::{
    AstAnnotation, AstExpr, AstField, AstFieldType, AstModel, AstSystem, FieldSelection,
    Identifier, LogicalOp, RelationalOp,
};

pub struct Scope {
    pub enclosing_model: Option<String>,
}

pub trait Typecheck<T> {
    fn shallow(&self) -> T;
    fn pass(&self, typ: &mut T, env: &MappedArena<Type>, scope: &Scope) -> bool;
}

pub fn populate_standard_env(env: &mut MappedArena<Type>) {
    env.add("Boolean", Type::Primitive(PrimitiveType::Boolean));
    env.add("Int", Type::Primitive(PrimitiveType::Int));
    env.add("String", Type::Primitive(PrimitiveType::String));
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Primitive(PrimitiveType),
    Composite(CompositeType),
    Optional(Box<Type>),
    List(Box<Type>),
    Reference(String),
    Defer,
    Error(String),
}

impl Type {
    pub fn is_defer(&self) -> bool {
        match &self {
            Type::Defer => true,
            Type::Optional(underlying) => underlying.deref().is_defer(),
            Type::List(underlying) => underlying.deref().is_defer(),
            _ => false,
        }
    }

    pub fn is_error(&self) -> bool {
        match &self {
            Type::Error(_) => true,
            Type::Optional(underlying) => underlying.deref().is_error(),
            Type::List(underlying) => underlying.deref().is_error(),
            _ => false,
        }
    }

    pub fn is_incomplete(&self) -> bool {
        self.is_defer() || self.is_error()
    }

    pub fn deref<'a>(&'a self, env: &'a MappedArena<Type>) -> Type {
        match &self {
            Type::Reference(name) => env.get_by_key(name).unwrap().clone(),
            Type::Optional(underlying) => Type::Optional(Box::new(underlying.deref().deref(env))),
            Type::List(underlying) => Type::List(Box::new(underlying.deref().deref(env))),
            o => o.deref().clone(),
        }
    }

    pub fn as_primitive(&self) -> PrimitiveType {
        match &self {
            Type::Primitive(p) => p.clone(),
            _ => panic!("Not a primitive: {:?}", self),
        }
    }

    // useful for relation creation
    pub fn inner_composite<'a>(&'a self, env: &'a MappedArena<Type>) -> &'a CompositeType {
        match &self {
            Type::Composite(c) => c,
            Type::Reference(r) => env.get_by_key(r).unwrap().inner_composite(env),
            Type::Optional(o) => o.inner_composite(env),
            Type::List(o) => o.inner_composite(env),
            _ => panic!("Cannot get inner composite of type {:?}", self),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeType {
    pub name: String,
    pub fields: Vec<TypedField>,
    pub annotations: Vec<TypedAnnotation>,
}

impl CompositeType {
    pub fn get_annotation(&self, name: &str) -> Option<&TypedAnnotation> {
        self.annotations.iter().find(|a| a.name == *name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveType {
    Int,
    String,
    Boolean,
}

impl PrimitiveType {
    pub fn to_column_type(&self) -> PhysicalColumnType {
        match &self {
            PrimitiveType::Int => PhysicalColumnType::Int { bits: IntBits::_32 },
            PrimitiveType::String => PhysicalColumnType::String,
            PrimitiveType::Boolean => PhysicalColumnType::Boolean,
        }
    }
}

impl Typecheck<Type> for AstFieldType {
    fn shallow(&self) -> Type {
        match &self {
            AstFieldType::Plain(_) => Type::Defer,
            AstFieldType::Optional(u) => Type::Optional(Box::new(u.shallow())),
            AstFieldType::List(u) => Type::List(Box::new(u.shallow())),
        }
    }

    fn pass(&self, typ: &mut Type, env: &MappedArena<Type>, scope: &Scope) -> bool {
        if typ.is_incomplete() {
            match &self {
                AstFieldType::Plain(name) => {
                    if env.get_id(name.as_str()).is_some() {
                        *typ = Type::Reference(name.clone());
                        true
                    } else {
                        *typ = Type::Error(format!("Unknown type: {}", name));
                        false
                    }
                }

                AstFieldType::Optional(inner_ast) => {
                    if let Type::Optional(inner_typ) = typ {
                        inner_ast.pass(inner_typ, env, scope)
                    } else {
                        panic!()
                    }
                }

                AstFieldType::List(inner_ast) => {
                    if let Type::List(inner_typ) = typ {
                        inner_ast.pass(inner_typ, env, scope)
                    } else {
                        panic!()
                    }
                }
            }
        } else {
            false
        }
    }
}

impl Typecheck<Type> for AstModel {
    fn shallow(&self) -> Type {
        Type::Composite(CompositeType {
            name: self.name.clone(),
            fields: self.fields.iter().map(|f| f.shallow()).collect(),
            annotations: self.annotations.iter().map(|a| a.shallow()).collect(),
        })
    }

    fn pass(&self, typ: &mut Type, env: &MappedArena<Type>, _scope: &Scope) -> bool {
        if let Type::Composite(c) = typ {
            let model_scope = Scope {
                enclosing_model: Some(self.name.clone()),
            };
            let fields_changed = self
                .fields
                .iter()
                .zip(c.fields.iter_mut())
                .map(|(f, tf)| f.pass(tf, env, &model_scope))
                .filter(|v| *v)
                .count()
                > 0;
            fields_changed
        } else {
            panic!()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedField {
    pub name: String,
    pub typ: Type,
    pub annotations: Vec<TypedAnnotation>,
}

impl TypedField {
    pub fn get_annotation(&self, name: &str) -> Option<&TypedAnnotation> {
        self.annotations.iter().find(|a| a.name == *name)
    }
}

impl Typecheck<TypedField> for AstField {
    fn shallow(&self) -> TypedField {
        TypedField {
            name: self.name.clone(),
            typ: self.typ.shallow(),
            annotations: self.annotations.iter().map(|a| a.shallow()).collect(),
        }
    }

    fn pass(&self, typ: &mut TypedField, env: &MappedArena<Type>, scope: &Scope) -> bool {
        let typ_changed = self.typ.pass(&mut typ.typ, env, scope);

        let annot_changed = self
            .annotations
            .iter()
            .zip(typ.annotations.iter_mut())
            .map(|(f, tf)| f.pass(tf, env, scope))
            .filter(|v| *v)
            .count()
            > 0;

        typ_changed || annot_changed
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedAnnotation {
    pub name: String,
    pub params: Vec<TypedExpression>,
}

impl Typecheck<TypedAnnotation> for AstAnnotation {
    fn shallow(&self) -> TypedAnnotation {
        TypedAnnotation {
            name: self.name.clone(),
            params: self.params.iter().map(|p| p.shallow()).collect(),
        }
    }

    fn pass(&self, typ: &mut TypedAnnotation, env: &MappedArena<Type>, scope: &Scope) -> bool {
        let params_changed = self
            .params
            .iter()
            .zip(typ.params.iter_mut())
            .map(|(p, p_typ)| p.pass(p_typ, env, scope))
            .filter(|c| *c)
            .count()
            > 0;
        params_changed
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypedExpression {
    FieldSelection(TypedFieldSelection),
    LogicalOp(TypedLogicalOp),
    RelationalOp(TypedRelationalOp),
    StringLiteral(String, Type),
}

impl TypedExpression {
    pub fn typ(&self) -> &Type {
        match &self {
            TypedExpression::FieldSelection(select) => select.typ(),
            TypedExpression::LogicalOp(logic) => logic.typ(),
            TypedExpression::RelationalOp(relation) => relation.typ(),
            TypedExpression::StringLiteral(_, t) => t,
        }
    }

    pub fn as_string(&self) -> String {
        match &self {
            TypedExpression::StringLiteral(s, _) => s.clone(),
            _ => panic!(),
        }
    }
}

impl Typecheck<TypedExpression> for AstExpr {
    fn shallow(&self) -> TypedExpression {
        match &self {
            AstExpr::FieldSelection(select) => TypedExpression::FieldSelection(select.shallow()),
            AstExpr::LogicalOp(logic) => TypedExpression::LogicalOp(logic.shallow()),
            AstExpr::RelationalOp(relation) => TypedExpression::RelationalOp(relation.shallow()),
            AstExpr::StringLiteral(v) => {
                TypedExpression::StringLiteral(v.clone(), Type::Primitive(PrimitiveType::String))
            }
        }
    }

    fn pass(&self, typ: &mut TypedExpression, env: &MappedArena<Type>, scope: &Scope) -> bool {
        match &self {
            AstExpr::FieldSelection(select) => {
                if let TypedExpression::FieldSelection(select_typ) = typ {
                    select.pass(select_typ, env, scope)
                } else {
                    panic!()
                }
            }
            AstExpr::LogicalOp(logic) => {
                if let TypedExpression::LogicalOp(logic_typ) = typ {
                    logic.pass(logic_typ, env, scope)
                } else {
                    panic!()
                }
            }
            AstExpr::RelationalOp(relation) => {
                if let TypedExpression::RelationalOp(relation_typ) = typ {
                    relation.pass(relation_typ, env, scope)
                } else {
                    panic!()
                }
            }
            AstExpr::StringLiteral(_) => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TypedFieldSelection {
    Single(Identifier, Type),
    Select(Box<TypedFieldSelection>, Identifier, Type),
}

impl TypedFieldSelection {
    pub fn typ(&self) -> &Type {
        match &self {
            TypedFieldSelection::Single(_, typ) => typ,
            TypedFieldSelection::Select(_, _, typ) => typ,
        }
    }
}

impl Typecheck<TypedFieldSelection> for FieldSelection {
    fn shallow(&self) -> TypedFieldSelection {
        match &self {
            FieldSelection::Single(v) => TypedFieldSelection::Single(v.clone(), Type::Defer),
            FieldSelection::Select(selection, i) => {
                TypedFieldSelection::Select(Box::new(selection.shallow()), i.clone(), Type::Defer)
            }
        }
    }

    fn pass(&self, typ: &mut TypedFieldSelection, env: &MappedArena<Type>, scope: &Scope) -> bool {
        match &self {
            FieldSelection::Single(Identifier(i)) => {
                if let TypedFieldSelection::Single(_, Type::Defer) = typ {
                    if i.as_str() == "self" {
                        if let Some(enclosing) = &scope.enclosing_model {
                            *typ = TypedFieldSelection::Single(
                                Identifier(i.clone()),
                                Type::Reference(enclosing.clone()),
                            );
                            true
                        } else {
                            *typ = TypedFieldSelection::Single(
                                Identifier(i.clone()),
                                Type::Error("Cannot use self outside a model".to_string()),
                            );
                            false
                        }
                    } else {
                        *typ = TypedFieldSelection::Single(
                            Identifier(i.clone()),
                            Type::Error(format!("Reference to unknown value: {}", i)),
                        );
                        false
                    }
                } else {
                    false
                }
            }
            FieldSelection::Select(selection, i) => {
                if let TypedFieldSelection::Select(prefix, _, typ) = typ {
                    let in_updated = selection.pass(prefix, env, scope);
                    let out_updated = if typ.is_incomplete() {
                        if let Type::Composite(c) = prefix.typ().deref(env) {
                            if let Some(field) = c.fields.iter().find(|f| f.name == i.0) {
                                if !field.typ.is_incomplete() {
                                    assert!(*typ != field.typ.clone());
                                    *typ = field.typ.clone();
                                    true
                                } else {
                                    *typ = Type::Error(format!(
                                        "Cannot read field {} in model {:?} with incomplete type",
                                        i.0,
                                        prefix.typ()
                                    ));
                                    false
                                }
                            } else {
                                *typ = Type::Error(format!("No such field: {}", i.0));
                                false
                            }
                        } else {
                            *typ = Type::Error(format!(
                                "Cannot read field {} from a non-composite type {:?}",
                                i.0,
                                prefix.typ()
                            ));
                            false
                        }
                    } else {
                        false
                    };

                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TypedLogicalOp {
    Not(Box<TypedExpression>, Type),
    And(Box<TypedExpression>, Box<TypedExpression>, Type),
    Or(Box<TypedExpression>, Box<TypedExpression>, Type),
}

impl TypedLogicalOp {
    pub fn typ(&self) -> &Type {
        match &self {
            TypedLogicalOp::Not(_, typ) => typ,
            TypedLogicalOp::And(_, _, typ) => typ,
            TypedLogicalOp::Or(_, _, typ) => typ,
        }
    }
}

impl Typecheck<TypedLogicalOp> for LogicalOp {
    fn shallow(&self) -> TypedLogicalOp {
        match &self {
            LogicalOp::Not(v) => TypedLogicalOp::Not(Box::new(v.shallow()), Type::Defer),
            LogicalOp::And(left, right) => TypedLogicalOp::And(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            LogicalOp::Or(left, right) => TypedLogicalOp::Or(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
        }
    }

    fn pass(&self, typ: &mut TypedLogicalOp, env: &MappedArena<Type>, scope: &Scope) -> bool {
        match &self {
            LogicalOp::Not(v) => {
                if let TypedLogicalOp::Not(v_typ, o_typ) = typ {
                    let in_updated = v.pass(v_typ, env, scope);
                    let out_updated = if o_typ.is_incomplete() {
                        if v_typ.typ().deref(env) == Type::Primitive(PrimitiveType::Boolean) {
                            *o_typ = Type::Primitive(PrimitiveType::Boolean);
                            true
                        } else {
                            *o_typ = Type::Error(format!(
                                "Cannot negate non-boolean type {:?}",
                                v_typ.typ().deref(env)
                            ));
                            false
                        }
                    } else {
                        false
                    };
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            LogicalOp::And(left, right) => {
                if let TypedLogicalOp::And(left_typ, right_typ, o_typ) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = if o_typ.is_incomplete() {
                        if left_typ.typ().deref(env) == Type::Primitive(PrimitiveType::Boolean)
                            && right_typ.typ().deref(env) == Type::Primitive(PrimitiveType::Boolean)
                        {
                            *o_typ = Type::Primitive(PrimitiveType::Boolean);
                            true
                        } else {
                            *o_typ = Type::Error("Both inputs to && must be booleans".to_string());
                            false
                        }
                    } else {
                        false
                    };
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            LogicalOp::Or(left, right) => {
                if let TypedLogicalOp::Or(left_typ, right_typ, o_typ) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = if o_typ.is_incomplete() {
                        if left_typ.typ().deref(env) == Type::Primitive(PrimitiveType::Boolean)
                            && right_typ.typ().deref(env) == Type::Primitive(PrimitiveType::Boolean)
                        {
                            *o_typ = Type::Primitive(PrimitiveType::Boolean);
                            true
                        } else {
                            *o_typ = Type::Error("Both inputs to || must be booleans".to_string());
                            false
                        }
                    } else {
                        false
                    };
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TypedRelationalOp {
    Eq(Box<TypedExpression>, Box<TypedExpression>, Type),
    Neq(Box<TypedExpression>, Box<TypedExpression>, Type),
    Lt(Box<TypedExpression>, Box<TypedExpression>, Type),
    Lte(Box<TypedExpression>, Box<TypedExpression>, Type),
    Gt(Box<TypedExpression>, Box<TypedExpression>, Type),
    Gte(Box<TypedExpression>, Box<TypedExpression>, Type),
}

impl TypedRelationalOp {
    pub fn typ(&self) -> &Type {
        match &self {
            TypedRelationalOp::Eq(_, _, typ) => typ,
            TypedRelationalOp::Neq(_, _, typ) => typ,
            TypedRelationalOp::Lt(_, _, typ) => typ,
            TypedRelationalOp::Lte(_, _, typ) => typ,
            TypedRelationalOp::Gt(_, _, typ) => typ,
            TypedRelationalOp::Gte(_, _, typ) => typ,
        }
    }
}

impl Typecheck<TypedRelationalOp> for RelationalOp {
    fn shallow(&self) -> TypedRelationalOp {
        match &self {
            RelationalOp::Eq(left, right) => TypedRelationalOp::Eq(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            RelationalOp::Neq(left, right) => TypedRelationalOp::Neq(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            RelationalOp::Lt(left, right) => TypedRelationalOp::Lt(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            RelationalOp::Lte(left, right) => TypedRelationalOp::Lte(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            RelationalOp::Gt(left, right) => TypedRelationalOp::Gt(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
            RelationalOp::Gte(left, right) => TypedRelationalOp::Gte(
                Box::new(left.shallow()),
                Box::new(right.shallow()),
                Type::Defer,
            ),
        }
    }

    fn pass(&self, typ: &mut TypedRelationalOp, env: &MappedArena<Type>, scope: &Scope) -> bool {
        match &self {
            RelationalOp::Eq(left, right) => {
                if let TypedRelationalOp::Eq(left_typ, right_typ, o_typ) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = if o_typ.is_incomplete() {
                        if left_typ.typ().deref(env) == right_typ.typ().deref(env) {
                            *o_typ = Type::Primitive(PrimitiveType::Boolean);
                            true
                        } else {
                            *o_typ = Type::Error(format!(
                                "Mismatched types, comparing {:?} with {:?}",
                                left_typ.typ().deref(env),
                                right_typ.typ().deref(env)
                            ));
                            false
                        }
                    } else {
                        false
                    };
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Neq(left, right) => {
                if let TypedRelationalOp::Neq(left_typ, right_typ, _) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Lt(left, right) => {
                if let TypedRelationalOp::Lt(left_typ, right_typ, _) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Lte(left, right) => {
                if let TypedRelationalOp::Lte(left_typ, right_typ, _) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Gt(left, right) => {
                if let TypedRelationalOp::Gt(left_typ, right_typ, _) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
            RelationalOp::Gte(left, right) => {
                if let TypedRelationalOp::Gte(left_typ, right_typ, _) = typ {
                    let in_updated =
                        left.pass(left_typ, env, scope) || right.pass(right_typ, env, scope);
                    let out_updated = false;
                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
        }
    }
}

pub fn build(ast_system: AstSystem) -> MappedArena<Type> {
    let ast_types = &ast_system.models;

    let mut types_arena: MappedArena<Type> = MappedArena::default();
    populate_standard_env(&mut types_arena);
    for model in ast_types {
        types_arena.add(model.name.as_str(), model.shallow());
    }

    loop {
        let mut did_change = false;
        let init_scope = Scope {
            enclosing_model: None,
        };
        for model in ast_types {
            let orig = types_arena.get_by_key(model.name.as_str()).unwrap();
            let mut typ = types_arena.get_by_key(model.name.as_str()).unwrap().clone();
            let pass_res = model.pass(&mut typ, &types_arena, &init_scope);
            if pass_res {
                assert!(*orig != typ);
                *types_arena.get_by_key_mut(model.name.as_str()).unwrap() = typ;
                did_change = true;
            } else {
            }
        }

        if !did_change {
            break;
        }
    }

    types_arena
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::*;

    #[test]
    fn simple() {
        let src = r#"
      model User {
        doc: Doc @column("custom_column") @auth(self.role == "role_admin" || self.role == "role_superuser" || self.doc.is_public)
        role: String
      }

      model Doc {
        is_public: Boolean
      }
      "#;
        let parsed = parse_str(src);
        let checked = build(parsed);

        let mut types = Vec::new();
        let mut keys = checked.keys().collect::<Vec<&String>>();
        keys.sort();
        for key in keys.iter() {
            types.push((key, checked.get_by_key(key).unwrap()));
        }
        insta::assert_yaml_snapshot!(types);
    }
}
