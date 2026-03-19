// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::primitive_type::{self, PrimitiveType};

use crate::ast::ast_types::{AstAccessExpr, AstLiteral};

use super::{Type, Typed};

impl AstLiteral {
    pub fn typ(&self) -> Type {
        match self {
            AstLiteral::String(_, _) => {
                Type::Primitive(PrimitiveType::Plain(primitive_type::STRING_TYPE))
            }
            AstLiteral::Boolean(_, _) => {
                Type::Primitive(PrimitiveType::Plain(primitive_type::BOOLEAN_TYPE))
            }
            AstLiteral::Number(value, _) => {
                if value.parse::<i64>().is_ok() {
                    Type::Primitive(PrimitiveType::Plain(primitive_type::INT_TYPE))
                } else {
                    Type::Primitive(PrimitiveType::Plain(primitive_type::FLOAT_TYPE))
                }
            }
            AstLiteral::Null(_) => Type::Null,
        }
    }
}

impl AstAccessExpr<Typed> {
    pub fn typ(&self) -> Type {
        match &self {
            AstAccessExpr::FieldSelection(select) => select.typ().clone(),
            AstAccessExpr::LogicalOp(logic) => logic.typ().clone(),
            AstAccessExpr::RelationalOp(relation) => relation.typ().clone(),
            AstAccessExpr::Literal(lit) => lit.typ(),
        }
    }
}
