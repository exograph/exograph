use crate::database_error::DatabaseError;

use super::{
    select::Select, transaction::TransactionStepId, ExpressionBuilder, SQLBuilder,
    SQLParamContainer,
};
use maybe_owned::MaybeOwned;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct PhysicalColumn {
    pub table_name: String,
    pub column_name: String,
    pub typ: PhysicalColumnType,
    pub is_pk: bool, // Is this column a part of the PK for the table (TODO: Generalize into constraints)
    pub is_auto_increment: bool, // temporarily keeping it here until we revamp how we represent types and column attributes
    pub is_nullable: bool,       // should this type have a NOT NULL constraint or not?

    pub unique_constraints: Vec<String>, // optional names for unique constraints

    pub default_value: Option<String>, // the default constraint
}

impl std::fmt::Debug for PhysicalColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "Column: {}.{}",
            &self.table_name, &self.column_name
        ))
    }
}

impl Default for PhysicalColumn {
    fn default() -> Self {
        Self {
            table_name: Default::default(),
            column_name: Default::default(),
            typ: PhysicalColumnType::Blob,
            is_pk: false,
            is_auto_increment: false,
            is_nullable: true,
            unique_constraints: vec![],
            default_value: None,
        }
    }
}

impl ExpressionBuilder for PhysicalColumn {
    fn build(&self, builder: &mut SQLBuilder) {
        if !builder.in_plain_mode() {
            builder.push_identifier(&self.table_name);
            builder.push('.');
        }
        builder.push_identifier(&self.column_name);
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
    pub fn from_string(s: &str) -> Result<PhysicalColumnType, DatabaseError> {
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
                            return Err(DatabaseError::Validation(format!("unknown type {s}")));
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
                            Regex::new("NUMERIC\\((?P<precision>\\d+),?(?P<scale>\\d+)?\\)")
                                .map_err(|_| {
                                    DatabaseError::Validation("Invalid numeric column spec".into())
                                })?;
                        let captures = regex.captures(s).unwrap();

                        let precision = captures
                            .name("precision")
                            .and_then(|s| s.as_str().parse().ok());
                        let scale = captures.name("scale").and_then(|s| s.as_str().parse().ok());

                        PhysicalColumnType::Numeric { precision, scale }
                    } else {
                        return Err(DatabaseError::Validation(format!("unknown type {s}")));
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
                    .map(|p| format!("@precision({p})"))
                    .unwrap_or_default();

                let scale_part = scale.map(|s| format!("@scale({s})")).unwrap_or_default();

                format!(" {precision_part} {scale_part}")
            }),

            PhysicalColumnType::String { length } => (
                "String".to_string(),
                match length {
                    Some(length) => format!(" @length({length})"),
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
                    Some(precision) => format!(" @precision({precision})"),
                    None => "".to_string(),
                },
            ),

            PhysicalColumnType::Time { precision } => (
                "LocalTime".to_string(),
                match precision {
                    Some(precision) => format!(" @precision({precision})"),
                    None => "".to_string(),
                },
            ),

            PhysicalColumnType::Date => ("LocalDate".to_string(), "".to_string()),

            PhysicalColumnType::Json => ("Json".to_string(), "".to_string()),
            PhysicalColumnType::Blob => ("Blob".to_string(), "".to_string()),
            PhysicalColumnType::Uuid => ("Uuid".to_string(), "".to_string()),

            PhysicalColumnType::Array { typ } => {
                let (data_type, annotations) = typ.to_model();
                (format!("[{data_type}]"), annotations)
            }

            PhysicalColumnType::ColumnReference { ref_table_name, .. } => {
                (ref_table_name.clone(), "".to_string())
            }
        }
    }
}

/// A column in a table. Essentially `<column>` in a `select <column>, <column> from <table>`
#[derive(Debug, PartialEq)]
pub enum Column<'a> {
    Physical(&'a PhysicalColumn),
    Literal(SQLParamContainer),
    JsonObject(Vec<JsonObjectElement<'a>>),
    JsonAgg(Box<Column<'a>>),
    SelectionTableWrapper(Box<Select<'a>>),
    // TODO: Generalize the following to return any type of value, not just strings
    Constant(String), // Currently needed to have a query return __typename set to a constant value
    Star(Option<String>), // * (None) or "table_name".* (Some("table_name"))
    Null,
    Function {
        function_name: String,
        column: &'a PhysicalColumn,
    },
}

#[derive(Debug, PartialEq)]
pub struct JsonObjectElement<'a> {
    pub key: String,
    pub value: Column<'a>,
}

impl<'a> JsonObjectElement<'a> {
    pub fn new(key: String, value: Column<'a>) -> Self {
        Self { key, value }
    }
}

/// Build a SQL query for an element in a JSON object. The SQL expression will be `'<key>',
/// <value>`, where `<value>` is the SQL expression for the value of the JSON object element. The
/// value of the JSON object element is encoded as base64 if it is a blob, and as text if it is a
/// numeric.
impl<'a> ExpressionBuilder for JsonObjectElement<'a> {
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("'");
        builder.push_str(&self.key);
        builder.push_str("', ");

        if let Column::Physical(PhysicalColumn { typ, .. }) = self.value {
            match &typ {
                // encode blob fields in JSON objects as base64
                // PostgreSQL inserts newlines into encoded base64 every 76 characters when in aligned mode
                // need to filter out using translate(...) function
                PhysicalColumnType::Blob => {
                    builder.push_str("translate(encode(");
                    self.value.build(builder);
                    builder.push_str(", \'base64\'), E'\\n', '')");
                }

                // numerics must be outputted as text to avoid any loss in precision
                PhysicalColumnType::Numeric { .. } => {
                    self.value.build(builder);
                    builder.push_str("::text");
                }

                _ => self.value.build(builder),
            }
        } else {
            self.value.build(builder)
        }
    }
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
            Column::JsonObject(elems) => {
                builder.push_str("json_build_object(");
                builder.push_elems(elems, ", ");
                builder.push(')');
            }
            Column::JsonAgg(column) => {
                // coalesce to return an empty array if we have no matching entities
                builder.push_str("COALESCE(json_agg(");
                column.build(builder);
                builder.push_str("), '[]'::json)");
            }
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
