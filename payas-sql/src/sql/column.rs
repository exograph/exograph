use super::{select::*, Expression, ExpressionContext, ParameterBinding, SQLParam};

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalColumn {
    pub table_name: String,
    pub column_name: String,
    pub typ: PhysicalColumnType,
    pub is_pk: bool, // Is this column a part of the PK for the table (TODO: Generalize into constraints)
    pub is_autoincrement: bool, // temporarily keeping it here until we revamp how we represent types and column attributes
    pub references: Option<ColumnReferece>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnReferece {
    pub table_name: String,
    pub column_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PhysicalColumnType {
    Int { bits: IntBits },
    String,
    Boolean,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IntBits {
    _16,
    _32,
    _64,
}

impl PhysicalColumnType {
    pub fn from_string(s: &str) -> PhysicalColumnType {
        match s {
            "Int" => PhysicalColumnType::Int { bits: IntBits::_32 },
            "String" => PhysicalColumnType::String,
            "Boolean" => PhysicalColumnType::Boolean,
            s => panic!("Unknown primitive type {}", s),
        }
    }

    pub fn db_type(&self, is_autoincrement: bool) -> &str {
        match self {
            PhysicalColumnType::Int { bits } => {
                if is_autoincrement {
                    match bits {
                        IntBits::_16 => "SMALLSERIAL",
                        IntBits::_32 => "SERIAL",
                        IntBits::_64 => "BIGSERIAL",
                    }
                } else {
                    match bits {
                        IntBits::_16 => "SMALLINT",
                        IntBits::_32 => "INT",
                        IntBits::_64 => "BIGINT",
                    }
                }
            }
            PhysicalColumnType::String => "TEXT",
            PhysicalColumnType::Boolean => "BOOLEAN",
        }
    }
}

#[derive(Debug)]
pub enum Column<'a> {
    Physical(&'a PhysicalColumn),
    Literal(Box<dyn SQLParam>),
    JsonObject(Vec<(String, &'a Column<'a>)>),
    JsonAgg(&'a Column<'a>),
    SelectionTableWrapper(Select<'a>),
    Constant(String), // Currently needed to have a query return __typename set to a constant value
    Star,
    Null,
}

// Due to https://github.com/rust-lang/rust/issues/39128, we have to manually implement PartialEq.
// If we try to put PartialEq in "derive" above, we get a "moving out... doesn't implement copy" error for the Literal variant
impl<'a> PartialEq for Column<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Column::Physical(v1), Column::Physical(v2)) => v1 == v2,
            (Column::Literal(v1), Column::Literal(v2)) => v1 == v2,
            (Column::JsonObject(v1), Column::JsonObject(v2)) => v1 == v2,
            (Column::JsonAgg(v1), Column::JsonAgg(v2)) => v1 == v2,
            (Column::SelectionTableWrapper(v1), Column::SelectionTableWrapper(v2)) => v1 == v2,
            (Column::Constant(v1), Column::Constant(v2)) => v1 == v2,
            (Column::Star, Column::Star) => true,
            (Column::Null, Column::Null) => true,
            _ => false,
        }
    }
}

impl<'a> Expression for Column<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        match self {
            Column::Physical(PhysicalColumn {
                table_name,
                column_name,
                ..
            }) => {
                let col_stmt = if expression_context.plain {
                    format!("\"{}\"", column_name)
                } else {
                    format!("\"{}\".\"{}\"", table_name, column_name)
                };
                ParameterBinding::new(col_stmt, vec![])
            }
            Column::Literal(value) => {
                let param_index = expression_context.next_param();
                ParameterBinding::new(format! {"${}", param_index}, vec![value])
            }
            Column::JsonObject(elems) => {
                let (elem_stmt, elem_params): (Vec<_>, Vec<_>) = elems
                    .iter()
                    .map(|elem| {
                        let elem_binding = elem.1.binding(expression_context);
                        (
                            format!("'{}', {}", elem.0, elem_binding.stmt),
                            elem_binding.params,
                        )
                    })
                    .unzip();

                let stmt = format!("json_build_object({})", elem_stmt.join(", "));
                let params = elem_params.into_iter().flatten().collect();
                ParameterBinding::new(stmt, params)
            }
            Column::JsonAgg(column) => {
                // coalesce to return an empty array if we have no matching enities
                let column_binding = column.binding(expression_context);
                let stmt = format!("coalesce(json_agg({}), '[]'::json)", column_binding.stmt);
                ParameterBinding::new(stmt, column_binding.params)
            }
            Column::SelectionTableWrapper(selection_table) => {
                let pb = selection_table.binding(expression_context);
                ParameterBinding::new(format!("({})", pb.stmt), pb.params)
            }
            Column::Constant(value) => ParameterBinding::new(format!("'{}'", value), vec![]),
            Column::Star => ParameterBinding::new("*".to_string(), vec![]),
            Column::Null => ParameterBinding::new("NULL".to_string(), vec![]),
        }
    }
}
