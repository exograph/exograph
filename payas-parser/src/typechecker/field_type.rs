use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::AstFieldType;

use super::{Scope, Type, Typecheck};

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
