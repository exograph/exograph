// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::Database;

use crate::core::pg_extension::{ArrayParamWrapper, PgExtension};
use crate::{ExpressionBuilder, SQLBuilder};

impl ExpressionBuilder for PgExtension {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match self {
            PgExtension::ArrayParam { param, wrapper } => {
                let wrapper_string = match wrapper {
                    ArrayParamWrapper::Any => "ANY",
                    ArrayParamWrapper::All => "ALL",
                    ArrayParamWrapper::None => "",
                };

                if wrapper_string.is_empty() {
                    builder.push_param(param.param());
                } else {
                    builder.push_str(wrapper_string);
                    builder.push('(');
                    builder.push_param(param.param());
                    builder.push(')');
                }
            }
            PgExtension::JsonObject(obj) => {
                obj.build(database, builder);
            }
            PgExtension::JsonAgg(agg) => agg.build(database, builder),
            // PgExtension is a flat enum shared across Column, Function, and OrderBy.
            // Only column variants (ArrayParam, JsonObject, JsonAgg) are valid here.
            // TODO: Refactor PgExtension into separate enums for ColumnExtension, FunctionExtension, and OrderByExtension to enforce this at the type level.
            PgExtension::VectorDistanceFunction { .. } | PgExtension::VectorDistance(..) => {
                unreachable!("Non-column PgExtension variant used in Column::Extension")
            }
        }
    }
}
