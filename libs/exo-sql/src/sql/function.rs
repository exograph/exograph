use crate::{ColumnId, Database, PhysicalColumnTypeExt, SQLParamContainer, VectorDistanceFunction};

use super::{ExpressionBuilder, SQLBuilder};

#[derive(Debug, PartialEq, Clone)]
pub enum Function {
    Named {
        function_name: String,
        column_id: ColumnId,
    },
    VectorDistance {
        column_id: ColumnId,
        distance_function: VectorDistanceFunction,
        target: SQLParamContainer,
    },
}

impl ExpressionBuilder for Function {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match self {
            Function::Named {
                function_name,
                column_id,
            } => {
                builder.push_str(function_name);
                builder.push('(');
                let column = column_id.get_column(database);
                column.build(database, builder);
                builder.push(')');
                if column
                    .typ
                    .is::<crate::sql::physical_column_type::VectorColumnType>()
                    && function_name != "count"
                {
                    // For vectors, we need to cast the result to a real array (otherwise it will be a string)
                    builder.push_str("::real[]");
                }
            }
            Function::VectorDistance {
                column_id,
                distance_function,
                target,
            } => {
                let column = column_id.get_column(database);
                column.build(database, builder);
                builder.push_space();
                distance_function.build(database, builder);
                builder.push_space();
                builder.push_param(target.param());
                builder.push_str("::vector");
            }
        }
    }
}
