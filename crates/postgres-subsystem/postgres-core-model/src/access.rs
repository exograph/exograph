// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::{
    access::{AccessPredicateExpression, CommonAccessPrimitiveExpression, FunctionCall},
    mapped_arena::SerializableSlabIndex,
};
use core_resolver::access_solver::AccessSolverError;

use exo_sql::PhysicalColumnPath;
use serde::{Deserialize, Serialize};

use crate::types::PostgresFieldDefaultValue;

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
#[derive(Serialize, Deserialize, Debug, Clone)]
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
    Normal(Vec<String>, Option<PostgresFieldDefaultValue>), // Non-pk field path such as self.title
    Pk {
        // pk field path such as self.project.owner.id
        lead: Vec<String>,                               // project
        lead_default: Option<PostgresFieldDefaultValue>, // default value for the lead field
        pk_fields: Vec<String>,                          // id (pk fields of Project)
    },
}

impl AccessPrimitiveExpressionPath {
    pub fn new(column_path: PhysicalColumnPath, field_path: FieldPath) -> Self {
        Self {
            column_path,
            field_path,
        }
    }

    pub fn with_function_context(
        self,
        other: Self,
        parameter_name: String,
    ) -> Result<Self, AccessSolverError> {
        Ok(Self {
            column_path: self.column_path.join(other.column_path),
            field_path: match other.field_path {
                FieldPath::Normal(b, default) => {
                    let mut new_field_path = vec![parameter_name.to_string()];
                    new_field_path.extend(b);
                    FieldPath::Normal(new_field_path, default)
                }
                FieldPath::Pk {
                    lead,
                    lead_default,
                    pk_fields,
                } => {
                    let mut new_field_path = vec![parameter_name.to_string()];
                    new_field_path.extend(lead);
                    FieldPath::Pk {
                        lead: new_field_path,
                        lead_default,
                        pk_fields: pk_fields.clone(),
                    }
                }
            },
        })
    }
}
