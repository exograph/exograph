use super::{
    json_agg::JsonAgg, json_object::JsonObject, physical_column::PhysicalColumn, select::Select,
    transaction::TransactionStepId, ExpressionBuilder, SQLBuilder, SQLParamContainer,
};
use maybe_owned::MaybeOwned;

/// A column in a table. Essentially `<column>` in a `select <column>, <column> from <table>`
#[derive(Debug, PartialEq)]
pub enum Column<'a> {
    Physical(&'a PhysicalColumn),
    Literal(SQLParamContainer),
    JsonObject(JsonObject<'a>),
    JsonAgg(JsonAgg<'a>),
    SelectionTableWrapper(Box<Select<'a>>),
    // TODO: Generalize the following to return any type of value, not just strings
    /// Needed to have a query return __typename set to a constant value
    Constant(String),
    /// All columns of a tables. If the table is None should translate to `*`, else  "table_name".*
    Star(Option<String>),
    Null,
    Function {
        function_name: String,
        column: &'a PhysicalColumn,
    },
}

impl<'a> ExpressionBuilder for Column<'a> {
    fn build(&self, builder: &mut SQLBuilder) {
        match self {
            Column::Physical(pc) => pc.build(builder),
            Column::Function {
                function_name,
                column,
            } => {
                builder.push_str(function_name);
                builder.push('(');
                column.build(builder);
                builder.push(')');
            }
            Column::Literal(value) => builder.push_param(value.param()),
            Column::JsonObject(obj) => {
                obj.build(builder);
            }
            Column::JsonAgg(agg) => agg.build(builder),
            Column::SelectionTableWrapper(selection_table) => {
                builder.push('(');
                selection_table.build(builder);
                builder.push(')');
            }
            Column::Constant(value) => {
                builder.push('\'');
                builder.push_str(value);
                builder.push('\'');
            }
            Column::Star(table_name) => {
                if let Some(table_name) = table_name {
                    builder.push_identifier(table_name);
                    builder.push('.');
                }
                builder.push('*');
            }
            Column::Null => {
                builder.push_str("NULL");
            }
        }
    }
}

#[derive(Debug)]
pub enum ProxyColumn<'a> {
    Concrete(MaybeOwned<'a, Column<'a>>),
    Template {
        col_index: usize,
        step_id: TransactionStepId,
    },
}
