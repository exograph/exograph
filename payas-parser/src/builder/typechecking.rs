use id_arena::{Arena, Id};
use serde::{Serialize, Deserialize, Serializer};
use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstExpr, AstField, AstModel, AstSystem};

pub trait Typecheck<T> {
  fn shallow(&self) -> T;
  fn pass(&self, typ: &mut T, env: &MappedArena<Type>) -> bool;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Primitive(PrimitiveType),
    Composite {
      name: String,
      fields: Vec<TypedField>,
      annotations: Vec<TypedAnnotation>
    },
    Reference(String),
    Defer
}

impl Type {
  pub fn deref(&self, env: &MappedArena<Type>) -> Type {
    todo!();
  }
}

impl Typecheck<Type> for AstModel {
  fn shallow(&self) -> Type {
    Type::Composite {
      name: self.name.clone(),
      fields: self.fields.iter().map(|f| f.shallow()).collect(),
      annotations: self.annotations.iter().map(|a| {
        TypedAnnotation {
          name: a.name.clone(),
          params: a.params.iter().map(|p| TypedExpression {
            expr: p.clone(),
            typ: Type::Defer
          }).collect()
        }
      }).collect()
    }
  }

  fn pass(&self, typ: &mut Type, env: &MappedArena<Type>) -> bool {
    if let Type::Composite { fields, .. } = typ {
      let fields_changed = self.fields.iter().zip(fields.iter_mut())
        .map(|(f, tf)| f.pass(tf, env)).any(|v| v);
      fields_changed
    } else { panic!() }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveType {
  INTEGER, STRING, BOOLEAN
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedField {
  name: String,
  typ: Type,
  annotations: Vec<TypedAnnotation>
}

impl Typecheck<TypedField> for AstField {
  fn shallow(&self) -> TypedField {
    TypedField {
      name: self.name.clone(),
      typ: Type::Defer,
      annotations: self.annotations.iter().map(|a| {
        TypedAnnotation {
          name: a.name.clone(),
          params: a.params.iter().map(|p| TypedExpression {
            expr: p.clone(),
            typ: Type::Defer
          }).collect()
        }
      }).collect()
    }
  }

  fn pass(&self, typ: &mut TypedField, env: &MappedArena<Type>) -> bool {
    let typ_changed = if typ.typ == Type::Defer {
      if let Some(field_typ) = env.get_id(self.typ.name().as_str()) {
        typ.typ = Type::Reference(self.typ.name());
        true
      } else { false }
    } else { false };
    typ_changed
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedAnnotation {
  name: String,
  params: Vec<TypedExpression>
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedExpression {
  expr: AstExpr,
  typ: Type
}

pub fn build(ast_system: AstSystem) -> MappedArena<Type> {
  let ast_types = &ast_system.models;

  let mut types_arena: MappedArena<Type> = MappedArena::default();
  for model in ast_types {
      types_arena.add(model.name.as_str(), model.shallow());
  }

  loop {
    let mut did_change = false;
    for model in ast_types {
      let mut typ = types_arena.get_by_key_mut(model.name.as_str()).unwrap().clone();
      let pass_res = model.pass(&mut typ, &types_arena);
      if pass_res {
        *types_arena.get_by_key_mut(model.name.as_str()).unwrap() = typ;
        did_change = true;
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
        document: Doc @column("custom_column") @auth(self.role == "role_admin" || self.role == "role_superuser")
        role: String
      }

      model Doc {

      }
      "#;
      let parsed = parse_str(src);
      let checked = build(parsed);
      insta::assert_yaml_snapshot!(checked.get_by_key("User").unwrap());
  }
}
