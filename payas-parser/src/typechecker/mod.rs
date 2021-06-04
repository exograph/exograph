mod annotation;
mod expression;
mod field;
mod field_type;
mod logical_op;
mod model;
mod relational_op;
mod selection;
mod typ;

pub(super) use annotation::TypedAnnotation;
pub(super) use expression::TypedExpression;
pub(super) use field::TypedField;
pub(super) use typ::{CompositeType, PrimitiveType, Type};

use crate::ast::ast_types::AstSystem;
use payas_model::model::mapped_arena::MappedArena;

pub struct Scope {
    pub enclosing_model: Option<String>,
}
pub trait Typecheck<T> {
    fn shallow(&self) -> T;
    fn pass(&self, typ: &mut T, env: &MappedArena<Type>, scope: &Scope) -> bool;
}

fn populate_standard_env(env: &mut MappedArena<Type>) {
    env.add("Boolean", Type::Primitive(PrimitiveType::Boolean));
    env.add("Int", Type::Primitive(PrimitiveType::Int));
    env.add("String", Type::Primitive(PrimitiveType::String));
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

    #[test]
    fn with_auth_context_use_in_field_annotation() {
        let src = r#"
        context AuthContext {
            role: String @jwt
        }
    
        model Doc {
          is_public: Boolean 
          content: String @access(AuthContext.role == "ROLE_ADMIN" || self.is_public)
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
