use exo_sql_core::{VectorDistanceFunction, physical_column::ColumnId};

use crate::sql_param_container::SQLParamContainer;

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
