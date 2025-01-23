// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::{
    core_model::{
        access::{AccessPredicateExpression, CommonAccessPrimitiveExpression, FunctionCall},
        mapped_arena::SerializableSlabIndex,
    },
    core_model_builder::error::ModelBuildingError,
};
use exo_sql::PhysicalColumnPath;
use serde::{Deserialize, Serialize};

/// Access specification for a model
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Access {
    pub creation: CreationAccessExpression,
    pub read: SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
    pub update: UpdateAccessExpression,
    pub delete: SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreationAccessExpression {
    pub precheck:
        SerializableSlabIndex<AccessPredicateExpression<PrecheckAccessPrimitiveExpression>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateAccessExpression {
    pub precheck:
        SerializableSlabIndex<AccessPredicateExpression<PrecheckAccessPrimitiveExpression>>,
    pub database:
        SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
}

/// Primitive expression (that doesn't contain any other expressions).
/// Used as sides of `AccessRelationalExpression` to form more complex expressions
/// such as equal and less than.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DatabaseAccessPrimitiveExpression {
    Column(PhysicalColumnPath, Option<String>), // Column path, for example self.user.id and parameter name (such as "du", default: "self")
    Function(PhysicalColumnPath, FunctionCall<Self>), // Function, for example self.documentUser.some(du => du.id == AuthContext.id && du.read)
    Common(CommonAccessPrimitiveExpression),          // expression shared by all access expressions
}

/// Primitive expressions that can express data input access control rules.
#[derive(Serialize, Deserialize, Debug)]
pub enum PrecheckAccessPrimitiveExpression {
    Path(AccessPrimitiveExpressionPath, Option<String>), // JSON path, for example self.user.id and parameter name (such as "du", default: "self")
    Function(AccessPrimitiveExpressionPath, FunctionCall<Self>), // Function, for example self.documentUser.some(du => du.id == AuthContext.id && du.read)
    Common(CommonAccessPrimitiveExpression), // expression shared by all access expressions
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AccessPrimitiveExpressionPath {
    pub column_path: PhysicalColumnPath,
    pub field_path: FieldPath,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FieldPath {
    Normal(Vec<String>), // Non-pk field path such as self.title
    Pk {
        // pk field path such as self.project.owner.id
        lead: Vec<String>,      // project
        pk_fields: Vec<String>, // id (pk fields of Project)
    },
}

impl AccessPrimitiveExpressionPath {
    pub fn new(column_path: PhysicalColumnPath, field_path: FieldPath) -> Self {
        Self {
            column_path,
            field_path,
        }
    }

    pub fn with_function_context(self, other: Self) -> Result<Self, ModelBuildingError> {
        Ok(Self {
            column_path: self.column_path.join(other.column_path),
            field_path: match (self.field_path, other.field_path) {
                (_, FieldPath::Normal(b)) => FieldPath::Normal(b.clone()),
                (_, FieldPath::Pk { lead, pk_fields }) => FieldPath::Pk {
                    lead: lead.clone(),
                    pk_fields: pk_fields.clone(),
                },
            },
        })
    }
}
