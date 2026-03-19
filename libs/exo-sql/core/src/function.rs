use crate::{
    physical_column::ColumnId, sql_param_container::SQLParamContainer,
    vector::VectorDistanceFunction,
};

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
