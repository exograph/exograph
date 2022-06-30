use crate::spec::SQLStatement;

use super::{
    select::Select, transaction::TransactionStepId, Expression, ExpressionContext,
    ParameterBinding, SQLParam,
};
use anyhow::{bail, Result};
use maybe_owned::MaybeOwned;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub struct PhysicalColumn {
    pub table_name: String,
    pub column_name: String,
    pub typ: PhysicalColumnType,
    pub is_pk: bool, // Is this column a part of the PK for the table (TODO: Generalize into constraints)
    pub is_autoincrement: bool, // temporarily keeping it here until we revamp how we represent types and column attributes
    pub is_nullable: bool,      // should this type have a NOT NULL constraint or not?

    pub unique_constraints: Vec<String>, // optional names for unique constraints

    pub default_value: Option<String>, // the default constraint
}

impl Default for PhysicalColumn {
    fn default() -> Self {
        Self {
            table_name: Default::default(),
            column_name: Default::default(),
            typ: PhysicalColumnType::Blob,
            is_pk: false,
            is_autoincrement: false,
            is_nullable: true,
            unique_constraints: vec![],
            default_value: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
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
    Blob,
    Uuid,
    Array {
        typ: Box<PhysicalColumnType>,
    },
    ColumnReference {
        ref_table_name: String,
        ref_column_name: String,
        ref_pk_type: Box<PhysicalColumnType>,
    },
    Float {
        bits: FloatBits,
    },
    Numeric {
        precision: Option<usize>,
        scale: Option<usize>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum IntBits {
    _16,
    _32,
    _64,
}

/// Number of bits in the float's mantissa.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum FloatBits {
    _24,
    _53,
}

impl PhysicalColumnType {
    /// Create a new physical column type given the SQL type string.
    pub fn from_string(s: &str) -> Result<PhysicalColumnType> {
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
                            bail!("unknown type {}", s);
                        }
                    } else {
                        break;
                    }
                }

                // Wrap the underlying type with `PhysicalColumnType::Array`
                let mut array_type = PhysicalColumnType::Array {
                    typ: Box::new(PhysicalColumnType::from_string(db_type)?),
                };
                for _ in 0..count - 1 {
                    array_type = PhysicalColumnType::Array {
                        typ: Box::new(array_type),
                    };
                }
                Ok(array_type)
            }

            None => Ok(match s.as_str() {
                // TODO: not really correct...
                "SMALLSERIAL" => PhysicalColumnType::Int { bits: IntBits::_16 },
                "SMALLINT" => PhysicalColumnType::Int { bits: IntBits::_16 },
                "INT" => PhysicalColumnType::Int { bits: IntBits::_32 },
                "INTEGER" => PhysicalColumnType::Int { bits: IntBits::_32 },
                "SERIAL" => PhysicalColumnType::Int { bits: IntBits::_32 },
                "BIGINT" => PhysicalColumnType::Int { bits: IntBits::_64 },
                "BIGSERIAL" => PhysicalColumnType::Int { bits: IntBits::_64 },

                "REAL" => PhysicalColumnType::Float {
                    bits: FloatBits::_24,
                },
                "DOUBLE PRECISION" => PhysicalColumnType::Float {
                    bits: FloatBits::_53,
                },

                "UUID" => PhysicalColumnType::Uuid,
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
                        || s.starts_with("CHAR")
                    {
                        PhysicalColumnType::String { length: get_num(s) }
                    } else if s.starts_with("TIMESTAMP") {
                        PhysicalColumnType::Timestamp {
                            precision: get_num(s),
                            timezone: s.contains("WITH TIME ZONE"),
                        }
                    } else if s.starts_with("TIME") {
                        PhysicalColumnType::Time {
                            precision: get_num(s),
                        }
                    } else if s.starts_with("DATE") {
                        PhysicalColumnType::Date
                    } else if s.starts_with("NUMERIC") {
                        let regex =
                            Regex::new("NUMERIC\\((?P<precision>\\d+),?(?P<scale>\\d+)?\\)")?;
                        let captures = regex.captures(s).unwrap();

                        let precision = captures
                            .name("precision")
                            .and_then(|s| s.as_str().parse().ok());
                        let scale = captures.name("scale").and_then(|s| s.as_str().parse().ok());

                        PhysicalColumnType::Numeric { precision, scale }
                    } else {
                        bail!("unknown type {}", s)
                    }
                }
            }),
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

            PhysicalColumnType::Float { bits } => (
                "Float".to_string(),
                match bits {
                    FloatBits::_24 => " @bits(24)",
                    FloatBits::_53 => " @bits(53)",
                }
                .to_owned(),
            ),

            PhysicalColumnType::Numeric { precision, scale } => ("Numeric".to_string(), {
                let precision_part = precision
                    .map(|p| format!("@precision({})", p))
                    .unwrap_or_default();

                let scale_part = scale.map(|s| format!("@scale({})", s)).unwrap_or_default();

                format!(" {} {}", precision_part, scale_part)
            }),

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
            PhysicalColumnType::Blob => ("Blob".to_string(), "".to_string()),
            PhysicalColumnType::Uuid => ("Uuid".to_string(), "".to_string()),

            PhysicalColumnType::Array { typ } => {
                let (data_type, annotations) = typ.to_model();
                (format!("[{}]", data_type), annotations)
            }

            PhysicalColumnType::ColumnReference { ref_table_name, .. } => {
                (ref_table_name.clone(), "".to_string())
            }
        }
    }

    pub fn to_sql(
        &self,
        table_name: &str,
        column_name: &str,
        is_autoincrement: bool,
    ) -> SQLStatement {
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
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Float { bits } => SQLStatement {
                statement: match bits {
                    FloatBits::_24 => "REAL",
                    FloatBits::_53 => "DOUBLE PRECISION",
                }
                .to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Numeric { precision, scale } => SQLStatement {
                statement: {
                    if let Some(p) = precision {
                        if let Some(s) = scale {
                            format!("NUMERIC({}, {})", p, s)
                        } else {
                            format!("NUMERIC({})", p)
                        }
                    } else {
                        assert!(scale.is_none()); // can't have a scale and no precision
                        "NUMERIC".to_owned()
                    }
                },
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::String { length } => SQLStatement {
                statement: if let Some(length) = length {
                    format!("VARCHAR({})", length)
                } else {
                    "TEXT".to_owned()
                },
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Boolean => SQLStatement {
                statement: "BOOLEAN".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
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
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Time { precision } => SQLStatement {
                statement: if let Some(p) = precision {
                    format!("TIME({})", p)
                } else {
                    "TIME".to_owned()
                },
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Date => SQLStatement {
                statement: "DATE".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Json => SQLStatement {
                statement: "JSONB".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Blob => SQLStatement {
                statement: "BYTEA".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
            },

            PhysicalColumnType::Uuid => SQLStatement {
                statement: "uuid".to_owned(),
                pre_statements: vec![],
                post_statements: vec![],
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

                let mut sql_statement =
                    underlying_typ.to_sql(table_name, column_name, is_autoincrement);
                sql_statement.statement += &dimensions_part;
                sql_statement
            }

            PhysicalColumnType::ColumnReference {
                ref_table_name,
                ref_pk_type,
                ..
            } => {
                let mut sql_statement =
                    ref_pk_type.to_sql(table_name, column_name, is_autoincrement);
                let foreign_constraint = format!(
                    r#"ALTER TABLE "{table_name}" ADD CONSTRAINT "{table_name}_{column_name}_fk" FOREIGN KEY ("{column_name}") REFERENCES "{ref_table_name}";"#,
                    table_name = table_name,
                    column_name = column_name,
                    ref_table_name = ref_table_name,
                );

                sql_statement.pre_statements.push(foreign_constraint);
                sql_statement
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Column<'a> {
    Physical(&'a PhysicalColumn),
    Literal(MaybeOwned<'a, Box<dyn SQLParam>>),
    JsonObject(Vec<(String, MaybeOwned<'a, Column<'a>>)>),
    JsonAgg(Box<MaybeOwned<'a, Column<'a>>>),
    SelectionTableWrapper(Box<Select<'a>>),
    Constant(String), // Currently needed to have a query return __typename set to a constant value
    Star,
    Null,
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
            Column::Literal(value) => {
                let param_index = expression_context.next_param();
                ParameterBinding::new(format! {"${}", param_index}, vec![value.as_ref()])
            }
            Column::JsonObject(elems) => {
                let (elem_stmt, elem_params): (Vec<_>, Vec<_>) = elems
                    .iter()
                    .map(|elem| {
                        let elem_binding = elem.1.binding(expression_context);
                        let mut stmt = elem_binding.stmt;

                        if let Column::Physical(PhysicalColumn { typ, .. }) = &elem.1.as_ref() {
                            stmt = match &typ {
                                // encode blob fields in JSON objects as base64
                                // PostgreSQL inserts newlines into encoded base64 every 76 characters when in aligned mode
                                // need to filter out using translate(...) function
                                PhysicalColumnType::Blob => {
                                    format!("translate(encode({}, \'base64\'), E'\\n', '')", stmt)
                                }

                                // numerics must be outputted as text to avoid any loss in precision
                                PhysicalColumnType::Numeric { .. } => format!("{}::text", stmt),
                                _ => stmt,
                            }
                        }

                        (format!("'{}', {}", elem.0, stmt), elem_binding.params)
                    })
                    .unzip();

                let stmt = format!("json_build_object({})", elem_stmt.join(", "));
                let params = elem_params.into_iter().flatten().collect();
                ParameterBinding::new(stmt, params)
            }
            Column::JsonAgg(column) => {
                // coalesce to return an empty array if we have no matching entities
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

#[derive(Debug)]
pub enum ProxyColumn<'a> {
    Concrete(MaybeOwned<'a, Column<'a>>),
    Template {
        col_index: usize,
        step_id: TransactionStepId,
    },
}
