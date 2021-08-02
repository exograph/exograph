use crate::spec::SQLStatement;

use super::{select::*, Expression, ExpressionContext, ParameterBinding, SQLParam};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PhysicalColumn {
    pub table_name: String,
    pub column_name: String,
    pub typ: PhysicalColumnType,
    pub is_pk: bool, // Is this column a part of the PK for the table (TODO: Generalize into constraints)
    pub is_autoincrement: bool, // temporarily keeping it here until we revamp how we represent types and column attributes
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PhysicalColumnType {
    Int {
        bits: IntBits,
    },
    String {
        length: Option<usize>,
    },
    Boolean,
    Timestamp {
        timezone: bool,
        precision: Option<usize>,
    },
    Date,
    Time {
        precision: Option<usize>,
    },
    Json,
    Array {
        typ: Box<PhysicalColumnType>,
    },
    ColumnReference {
        column_name: String,
        ref_table_name: String,
        ref_pk_type: Box<PhysicalColumnType>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IntBits {
    _16,
    _32,
    _64,
}

impl PhysicalColumnType {
    /// Create a new physical column type given the SQL type string.
    pub fn from_string(s: &str) -> PhysicalColumnType {
        let s = s.to_uppercase();

        match s.find('[') {
            // If the type contains `[`, then it's an array type
            Some(idx) => {
                let db_type = &s[..idx]; // The underlying data type (e.g. `INT` in `INT[][]`)
                let mut dims = &s[idx..]; // The array brackets (e.g. `[][]` in `INT[][]`)

                // Count how many `[]` exist in `dims` (how many dimensions does this array have)
                let mut count = 0;
                loop {
                    if !dims.is_empty() {
                        if dims.len() >= 2 && &dims[0..2] == "[]" {
                            dims = &dims[2..];
                            count += 1;
                        } else {
                            panic!("Unknown dbtype {}", s)
                        }
                    } else {
                        break;
                    }
                }

                // Wrap the underlying type with `PhysicalColumnType::Array`
                let mut array_type = PhysicalColumnType::Array {
                    typ: Box::new(PhysicalColumnType::from_string(db_type)),
                };
                for _ in 0..count - 1 {
                    array_type = PhysicalColumnType::Array {
                        typ: Box::new(array_type),
                    };
                }
                array_type
            }

            None => match s.as_str() {
                // TODO: not really correct...
                "SMALLSERIAL" => PhysicalColumnType::Int { bits: IntBits::_16 },
                "SMALLINT" => PhysicalColumnType::Int { bits: IntBits::_16 },
                "INT" => PhysicalColumnType::Int { bits: IntBits::_32 },
                "INTEGER" => PhysicalColumnType::Int { bits: IntBits::_32 },
                "SERIAL" => PhysicalColumnType::Int { bits: IntBits::_32 },
                "BIGINT" => PhysicalColumnType::Int { bits: IntBits::_64 },
                "BIGSERIAL" => PhysicalColumnType::Int { bits: IntBits::_64 },

                "TEXT" => PhysicalColumnType::String { length: None },
                "BOOLEAN" => PhysicalColumnType::Boolean,
                "JSONB" => PhysicalColumnType::Json,
                s => {
                    // parse types with arguments
                    // TODO: more robust parsing

                    let get_num = |s: &str| {
                        s.chars()
                            .filter(|c| c.is_numeric())
                            .collect::<String>()
                            .parse::<usize>()
                            .ok()
                    };

                    if s.starts_with("CHARACTER VARYING")
                        || s.starts_with("VARCHAR")
                        || s.starts_with("CHAR[")
                    {
                        return PhysicalColumnType::String { length: get_num(s) };
                    }

                    if s.starts_with("TIMESTAMP") {
                        return PhysicalColumnType::Timestamp {
                            precision: get_num(s),
                            timezone: s.contains("WITH TIME ZONE"),
                        };
                    }

                    if s.starts_with("TIME") {
                        return PhysicalColumnType::Time {
                            precision: get_num(s),
                        };
                    }

                    if s.starts_with("DATE") {
                        return PhysicalColumnType::Date;
                    }

                    panic!("Unknown dbtype {}", s)
                }
            },
        }
    }

    pub fn to_model(&self) -> (String, String) {
        match self {
            PhysicalColumnType::Int { bits } => (
                "Int".to_string(),
                match bits {
                    IntBits::_16 => " @bits(16)",
                    IntBits::_32 => "",
                    IntBits::_64 => " @bits(64)",
                }
                .to_string(),
            ),

            PhysicalColumnType::String { length } => (
                "String".to_string(),
                match length {
                    Some(length) => format!(" @length({})", length),
                    None => "".to_string(),
                },
            ),

            PhysicalColumnType::Boolean => ("Boolean".to_string(), "".to_string()),

            PhysicalColumnType::Timestamp {
                timezone,
                precision,
            } => (
                if *timezone {
                    "Instant"
                } else {
                    "LocalDateTime"
                }
                .to_string(),
                match precision {
                    Some(precision) => format!(" @precision({})", precision),
                    None => "".to_string(),
                },
            ),

            PhysicalColumnType::Time { precision } => (
                "LocalTime".to_string(),
                match precision {
                    Some(precision) => format!(" @precision({})", precision),
                    None => "".to_string(),
                },
            ),

            PhysicalColumnType::Date => ("LocalDate".to_string(), "".to_string()),

            PhysicalColumnType::Json => ("Json".to_string(), "".to_string()),

            PhysicalColumnType::Array { typ } => {
                let (data_type, annotations) = typ.to_model();
                (format!("[{}]", data_type), annotations)
            }

            PhysicalColumnType::ColumnReference { ref_table_name, .. } => {
                (ref_table_name.clone(), "".to_string())
            }
        }
    }

    pub fn to_sql(&self, table_name: &str, is_autoincrement: bool) -> SQLStatement {
        match self {
            PhysicalColumnType::Int { bits } => SQLStatement {
                statement: {
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
                .to_owned(),
                foreign_constraints: Vec::new(),
            },

            PhysicalColumnType::String { length } => SQLStatement {
                statement: if let Some(length) = length {
                    format!("VARCHAR({})", length)
                } else {
                    "TEXT".to_owned()
                },
                foreign_constraints: Vec::new(),
            },

            PhysicalColumnType::Boolean => SQLStatement {
                statement: "BOOLEAN".to_owned(),
                foreign_constraints: Vec::new(),
            },

            PhysicalColumnType::Timestamp {
                timezone,
                precision,
            } => SQLStatement {
                statement: {
                    let timezone_option = if *timezone {
                        "WITH TIME ZONE"
                    } else {
                        "WITHOUT TIME ZONE"
                    };
                    let precision_option = if let Some(p) = precision {
                        format!("({})", p)
                    } else {
                        String::default()
                    };

                    let typ = match self {
                        PhysicalColumnType::Timestamp { .. } => "TIMESTAMP",
                        PhysicalColumnType::Time { .. } => "TIME",
                        _ => panic!(),
                    };

                    // e.g. "TIMESTAMP(3) WITH TIME ZONE"
                    format!("{}{} {}", typ, precision_option, timezone_option)
                },
                foreign_constraints: Vec::new(),
            },

            PhysicalColumnType::Time { precision } => SQLStatement {
                statement: if let Some(p) = precision {
                    format!("TIME({})", p)
                } else {
                    "TIME".to_owned()
                },
                foreign_constraints: Vec::new(),
            },

            PhysicalColumnType::Date => SQLStatement {
                statement: "DATE".to_owned(),
                foreign_constraints: Vec::new(),
            },

            PhysicalColumnType::Json => SQLStatement {
                statement: "JSONB".to_owned(),
                foreign_constraints: Vec::new(),
            },

            PhysicalColumnType::Array { typ } => {
                // 'unwrap' nested arrays all the way to the underlying primitive type

                let mut underlying_typ = typ;
                let mut dimensions = 1;

                while let PhysicalColumnType::Array { typ } = &**underlying_typ {
                    underlying_typ = typ;
                    dimensions += 1;
                }

                // build dimensions

                let mut dimensions_part = String::new();

                for _ in 0..dimensions {
                    write!(&mut dimensions_part, "[]").unwrap();
                }

                let mut sql_statement = underlying_typ.to_sql(table_name, is_autoincrement);
                sql_statement.statement += &dimensions_part;
                sql_statement
            }

            PhysicalColumnType::ColumnReference {
                column_name,
                ref_table_name,
                ref_pk_type,
            } => {
                let mut sql_statement = ref_pk_type.to_sql(table_name, is_autoincrement);
                let foreign_constraint = format!(
                    "ALTER TABLE {table} ADD CONSTRAINT {ref_table}_fk FOREIGN KEY ({column}) REFERENCES {ref_table};",
                    table = table_name,
                    column = column_name,
                    ref_table = ref_table_name,
                );

                sql_statement.foreign_constraints.push(foreign_constraint);
                sql_statement
            }
        }
    }
}

#[derive(Debug)]
pub enum Column<'a> {
    Physical(&'a PhysicalColumn),
    Array(Vec<&'a Column<'a>>),
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

impl<'a> Expression for PhysicalColumn {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let col_stmt = if expression_context.plain {
            format!("\"{}\"", self.column_name)
        } else {
            format!("\"{}\".\"{}\"", self.table_name, self.column_name)
        };
        ParameterBinding::new(col_stmt, vec![])
    }
}

impl<'a> Expression for Column<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        match self {
            Column::Physical(pc) => pc.binding(expression_context),
            Column::Array(list) => {
                let (elem_stmt, elem_params): (Vec<_>, Vec<_>) = list
                    .iter()
                    .map(|elem| {
                        let elem_binding = elem.binding(expression_context);
                        (elem_binding.stmt, elem_binding.params)
                    })
                    .unzip();

                let stmt = format!("ARRAY[{}]", elem_stmt.join(", "));
                let params = elem_params.into_iter().flatten().collect();

                ParameterBinding::new(stmt, params)
            }
            Column::Literal(value) => {
                let param_index = expression_context.next_param();
                ParameterBinding::new(format! {"${}", param_index}, vec![value.as_ref()])
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
