// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::mapped_arena::MappedArena;

use crate::ast::ast_types::AstFieldType;

use super::{Type, Typed};

impl AstFieldType<Typed> {
    pub fn get_underlying_typename(&self, types: &MappedArena<Type>) -> Option<String> {
        match &self {
            AstFieldType::Plain(..) => self.to_typ(types).get_underlying_typename(types),
            AstFieldType::Optional(underlying) => underlying.get_underlying_typename(types),
        }
    }

    pub fn to_typ(&self, types: &MappedArena<Type>) -> Type {
        match &self {
            AstFieldType::Plain(_module, name, params, ok, _) => {
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

    pub fn module_name(&self) -> Option<String> {
        match &self {
            AstFieldType::Plain(module, name, params, _, _) => match name.as_str() {
                "Set" | "Array" => params[0].module_name(),
                _ => module.clone(),
            },
            AstFieldType::Optional(underlying) => underlying.module_name(),
        }
    }
}
