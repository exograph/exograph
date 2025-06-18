// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::primitive_type::{self, JsonType, PrimitiveType};

use crate::ast::ast_types::AstExpr;

use super::{Type, Typed};

impl AstExpr<Typed> {
    pub fn typ(&self) -> Type {
        match &self {
            AstExpr::FieldSelection(select) => select.typ().clone(),
            AstExpr::LogicalOp(logic) => logic.typ().clone(),
            AstExpr::RelationalOp(relation) => relation.typ().clone(),
            AstExpr::StringLiteral(_, _) => {
                Type::Primitive(PrimitiveType::Plain(primitive_type::STRING_TYPE))
            }
            AstExpr::BooleanLiteral(_, _) => {
                Type::Primitive(PrimitiveType::Plain(primitive_type::BOOLEAN_TYPE))
            }
            AstExpr::NumberLiteral(value, _) => {
                if value.parse::<i64>().is_ok() {
                    Type::Primitive(PrimitiveType::Plain(primitive_type::INT_TYPE))
                } else {
                    Type::Primitive(PrimitiveType::Plain(primitive_type::FLOAT_TYPE))
                }
            }
            AstExpr::StringList(_, _) => Type::Array(Box::new(Type::Primitive(
                PrimitiveType::Plain(primitive_type::STRING_TYPE),
            ))),
            AstExpr::NullLiteral(_) => Type::Null,
            AstExpr::ObjectLiteral(_, _) => Type::Primitive(PrimitiveType::Plain(&JsonType)),
        }
    }

    pub fn as_string(&self) -> String {
        match &self {
            AstExpr::StringLiteral(s, _) => s.clone(),
            _ => panic!(),
        }
    }

    pub fn as_int(&self) -> i64 {
        match &self {
            AstExpr::NumberLiteral(n, _) => n.parse::<i64>().unwrap(),
            _ => panic!(),
        }
    }

    pub fn as_float(&self) -> f64 {
        match &self {
            AstExpr::NumberLiteral(n, _) => n.parse::<f64>().unwrap(),
            _ => panic!(),
        }
    }

    pub fn as_boolean(&self) -> bool {
        match &self {
            AstExpr::BooleanLiteral(b, _) => *b,
            _ => panic!(),
        }
    }
}
