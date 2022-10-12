use payas_core_model::mapped_arena::MappedArena;

use crate::ast::ast_types::AstFieldType;

use super::{Type, Typed};

impl AstFieldType<Typed> {
    pub fn get_underlying_typename(&self, types: &MappedArena<Type>) -> Option<String> {
        match &self {
            AstFieldType::Plain(_, _, _, _) => self.to_typ(types).get_underlying_typename(types),
            AstFieldType::Optional(underlying) => underlying.get_underlying_typename(types),
        }
    }

    pub fn to_typ(&self, types: &MappedArena<Type>) -> Type {
        match &self {
            AstFieldType::Plain(name, params, ok, _) => {
                if !ok {
                    Type::Error
                } else {
                    match name.as_str() {
                        "Set" => Type::Set(Box::new(params[0].to_typ(types))),
                        "Array" => Type::Array(Box::new(params[0].to_typ(types))),
                        o => Type::Reference(types.get_id(o).unwrap()),
                    }
                }
            }
            AstFieldType::Optional(underlying) => {
                Type::Optional(Box::new(underlying.to_typ(types)))
            }
        }
    }
}
