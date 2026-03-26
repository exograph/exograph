// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::Database;

use crate::core::pg_extension::{ArrayParamWrapper, PgColumnExtension};
use crate::{ExpressionBuilder, SQLBuilder};

impl ExpressionBuilder for PgColumnExtension {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match self {
            PgColumnExtension::ArrayParam { param, wrapper } => {
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
            PgColumnExtension::JsonObject(obj) => {
                obj.build(database, builder);
            }
            PgColumnExtension::JsonAgg(agg) => agg.build(database, builder),
        }
    }
}
