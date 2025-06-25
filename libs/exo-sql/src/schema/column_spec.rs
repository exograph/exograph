// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use crate::database_error::DatabaseError;
use crate::sql::connect::database_client::DatabaseClient;
use crate::sql::physical_column_type::{
    ArrayColumnType, BlobColumnType, BooleanColumnType, DateColumnType, EnumColumnType, IntBits,
    IntColumnType, JsonColumnType, NumericColumnType, PhysicalColumnType, StringColumnType,
    TimeColumnType, TimestampColumnType, VectorColumnType,
};
use crate::{Database, PhysicalColumn, SchemaObjectName};

use super::DebugPrintTo;
use super::enum_spec::EnumSpec;
use super::issue::{Issue, WithIssues};
use super::op::SchemaOp;
use super::statement::SchemaStatement;
use super::table_spec::TableSpec;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ColumnSpec {
    pub name: String,
    pub typ: Box<dyn PhysicalColumnType>,
    pub reference_specs: Option<Vec<ColumnReferenceSpec>>,
    pub is_pk: bool,
    pub is_nullable: bool,
    pub unique_constraints: Vec<String>,
    pub default_value: Option<ColumnDefault>,
}

impl PartialEq for ColumnSpec {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.typ.equals(other.typ.as_ref())
            && self.reference_specs == other.reference_specs
            && self.is_pk == other.is_pk
            && self.is_nullable == other.is_nullable
            && self.unique_constraints == other.unique_constraints
            && self.default_value == other.default_value
    }
}

impl DebugPrintTo for ColumnSpec {
    fn debug_print_to<W: std::io::Write>(
        &self,
        writer: &mut W,
        indent: usize,
    ) -> std::io::Result<()> {
        let indent_str = " ".repeat(indent);

        let references = if let Some(ref_specs) = &self.reference_specs {
            ref_specs
                .iter()
                .map(|ref_spec| {
                    format!(
                        "[{}.{}, ({})]",
                        ref_spec.foreign_table_name.fully_qualified_name(),
                        ref_spec.foreign_pk_column_name,
                        ref_spec.group_name
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            "[]".to_string()
        };

        let mut attributes = Vec::new();
        if self.is_pk {
            attributes.push("PK");
        }
        if !self.is_nullable {
            attributes.push("NOT NULL");
        }
        if !self.unique_constraints.is_empty() {
            attributes.push("UNIQUE");
        }
        if self.default_value.is_some() {
            attributes.push("DEFAULT");
        }

        let attr_str = if attributes.is_empty() {
            "".to_string()
        } else {
            format!(" [{}]", attributes.join(", "))
        };

        writeln!(
            writer,
            "{}- {}: {} (references: {}){}",
            indent_str,
            self.name,
            self.typ.type_string(),
            references,
            attr_str
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ColumnDefault {
    Uuid(UuidGenerationMethod),
    CurrentTimestamp,
    CurrentDate,
    Text(String),
    VarChar(String),
    Boolean(bool),
    Number(String),
    Function(String),
    Enum(String),
    Autoincrement(ColumnAutoincrement),
    Date(String),
    Time(String),
    DateTime(String),
    Json(String),
    UuidLiteral(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum UuidGenerationMethod {
    GenRandomUuid,
    UuidGenerateV4,
}

impl ColumnDefault {
    pub fn is_autoincrement(&self) -> bool {
        matches!(self, ColumnDefault::Autoincrement(_))
    }

    fn handle_string_default(default_value: &str) -> Result<ColumnDefault, DatabaseError> {
        if default_value.ends_with(TEXT_TYPE_CAST_PREFIX) {
            let text_value =
                default_value[1..default_value.len() - TEXT_TYPE_CAST_PREFIX.len()].to_string();
            Ok(ColumnDefault::Text(text_value))
        } else if default_value.ends_with(CHARACTER_VARYING_TYPE_CAST_PREFIX) {
            let var_char_value = default_value
                [1..default_value.len() - CHARACTER_VARYING_TYPE_CAST_PREFIX.len()]
                .to_string();
            Ok(ColumnDefault::VarChar(var_char_value))
        } else {
            Err(DatabaseError::Generic(format!(
                "Invalid default value for string column: {}",
                default_value
            )))
        }
    }

    fn handle_uuid_default(default_value: &str) -> Result<ColumnDefault, DatabaseError> {
        if default_value == "gen_random_uuid()" {
            Ok(ColumnDefault::Uuid(UuidGenerationMethod::GenRandomUuid))
        } else if default_value == "uuid_generate_v4()" {
            Ok(ColumnDefault::Uuid(UuidGenerationMethod::UuidGenerateV4))
        } else {
            let value = if default_value.starts_with("'") && default_value.ends_with("'::uuid") {
                &default_value[1..default_value.len() - 7]
            } else if default_value.starts_with("'") && default_value.ends_with("'") {
                &default_value[1..default_value.len() - 1]
            } else {
                default_value
            };
            Ok(ColumnDefault::UuidLiteral(value.to_string()))
        }
    }

    fn handle_numeric_default(default_value: &str) -> Result<ColumnDefault, DatabaseError> {
        match default_value.parse() {
            Ok(value) => Ok(ColumnDefault::Number(value)),
            Err(_) => Ok(ColumnDefault::Function(default_value.to_string())),
        }
    }

    fn handle_boolean_default(default_value: &str) -> Result<ColumnDefault, DatabaseError> {
        Ok(ColumnDefault::Boolean(default_value == "true"))
    }

    fn handle_date_default(default_value: &str) -> Result<ColumnDefault, DatabaseError> {
        if default_value == "now()" || default_value == "CURRENT_DATE" {
            Ok(ColumnDefault::CurrentDate)
        } else {
            let value = if default_value.starts_with("'") && default_value.ends_with("'::date") {
                &default_value[1..default_value.len() - 7]
            } else if default_value.starts_with("'") && default_value.ends_with("'") {
                &default_value[1..default_value.len() - 1]
            } else {
                default_value
            };
            Ok(ColumnDefault::Date(value.to_string()))
        }
    }

    fn handle_time_default(default_value: &str) -> Result<ColumnDefault, DatabaseError> {
        let value = if let Some(stripped) = default_value.strip_prefix("'") {
            if let Some(quote_end) = stripped.find("'") {
                &default_value[1..quote_end + 1]
            } else {
                stripped
            }
        } else {
            default_value
        };
        Ok(ColumnDefault::Time(value.to_string()))
    }

    fn handle_timestamp_default(default_value: &str) -> Result<ColumnDefault, DatabaseError> {
        if default_value == "now()" || default_value == "CURRENT_TIMESTAMP" {
            Ok(ColumnDefault::CurrentTimestamp)
        } else {
            let value = if let Some(stripped) = default_value.strip_prefix("'") {
                if let Some(quote_end) = stripped.find("'") {
                    let timestamp_str = &default_value[1..quote_end + 1];
                    timestamp_str.replace(" ", "T")
                } else {
                    stripped.to_string()
                }
            } else {
                default_value.to_string()
            };
            Ok(ColumnDefault::DateTime(value))
        }
    }

    fn handle_json_default(default_value: &str) -> Result<ColumnDefault, DatabaseError> {
        let value = if default_value.len() >= 8
            && default_value.starts_with("'")
            && default_value.ends_with("'::json")
        {
            &default_value[1..default_value.len() - 7]
        } else if default_value.len() >= 2
            && default_value.starts_with("'")
            && default_value.ends_with("'")
        {
            &default_value[1..default_value.len() - 1]
        } else {
            default_value
        };
        Ok(ColumnDefault::Json(value.to_string()))
    }

    fn handle_enum_default(
        default_value: &str,
        enum_type: &EnumColumnType,
    ) -> Result<ColumnDefault, DatabaseError> {
        let enum_name = &enum_type.enum_name;
        let enum_name_str = match &enum_name.schema {
            Some(schema) => format!("{}.{}", schema, enum_name.name),
            None => enum_name.name.clone(),
        };
        let default_value = default_value
            .strip_prefix("'")
            .and_then(|s| s.strip_suffix(format!("'::{}", enum_name_str).as_str()))
            .unwrap_or(default_value);

        Ok(ColumnDefault::Enum(default_value.to_string()))
    }

    /// Converts a value read by the `COLUMNS_DEFAULT_QUERY` to a `ColumnDefault`.
    pub fn from_sql(
        default_value: String,
        db_type: Option<&dyn PhysicalColumnType>,
    ) -> Result<ColumnDefault, DatabaseError> {
        match db_type {
            Some(physical_type) => {
                if physical_type.as_any().is::<StringColumnType>() {
                    Self::handle_string_default(&default_value)
                } else if physical_type
                    .as_any()
                    .is::<crate::sql::physical_column_type::UuidColumnType>()
                {
                    Self::handle_uuid_default(&default_value)
                } else if physical_type.as_any().is::<IntColumnType>()
                    || physical_type
                        .as_any()
                        .is::<crate::sql::physical_column_type::FloatColumnType>()
                    || physical_type.as_any().is::<NumericColumnType>()
                {
                    Self::handle_numeric_default(&default_value)
                } else if physical_type.as_any().is::<BooleanColumnType>() {
                    Self::handle_boolean_default(&default_value)
                } else if physical_type.as_any().is::<DateColumnType>() {
                    Self::handle_date_default(&default_value)
                } else if physical_type.as_any().is::<TimeColumnType>() {
                    Self::handle_time_default(&default_value)
                } else if physical_type.as_any().is::<TimestampColumnType>() {
                    Self::handle_timestamp_default(&default_value)
                } else if physical_type.as_any().is::<JsonColumnType>() {
                    Self::handle_json_default(&default_value)
                } else if physical_type.as_any().is::<BlobColumnType>() {
                    Ok(ColumnDefault::Function(default_value))
                } else if let Some(enum_type) =
                    physical_type.as_any().downcast_ref::<EnumColumnType>()
                {
                    Self::handle_enum_default(&default_value, enum_type)
                } else {
                    Ok(ColumnDefault::Function(default_value))
                }
            }
            None => Ok(ColumnDefault::Function(default_value)),
        }
    }

    pub fn to_sql(&self) -> Option<String> {
        match self {
            ColumnDefault::Uuid(method) => match method {
                UuidGenerationMethod::GenRandomUuid => Some("gen_random_uuid()".to_string()),
                UuidGenerationMethod::UuidGenerateV4 => Some("uuid_generate_v4()".to_string()),
            },
            ColumnDefault::CurrentTimestamp => Some("now()".to_string()),
            ColumnDefault::CurrentDate => Some("now()".to_string()),
            ColumnDefault::Text(value) => Some(format!("'{value}'::text")),
            ColumnDefault::VarChar(value) => Some(format!("'{value}'::character varying")),
            ColumnDefault::Boolean(value) => Some(format!("{value}")),
            ColumnDefault::Number(value) => Some(value.to_string()),
            ColumnDefault::Function(value) => Some(value.clone()),
            ColumnDefault::Enum(value) => Some(format!("'{value}'")),
            ColumnDefault::Autoincrement(autoincrement) => match autoincrement {
                ColumnAutoincrement::Serial => None, // The type `SERIAL` takes care of the default value
                ColumnAutoincrement::Sequence { name } => {
                    Some(format!("nextval('{}'::regclass)", name.sql_name()))
                }
                ColumnAutoincrement::Identity { .. } => None,
            },
            ColumnDefault::Date(value) => Some(format!("'{value}'::date")),
            ColumnDefault::Time(value) => Some(format!("'{value}'::time")),
            ColumnDefault::DateTime(value) => Some(format!("'{value}'::timestamp")),
            ColumnDefault::Json(value) => Some(format!("'{value}'::json")),
            ColumnDefault::UuidLiteral(value) => Some(format!("'{value}'::uuid")),
        }
    }

    pub fn to_model(&self) -> Option<String> {
        match self {
            ColumnDefault::Uuid(method) => match method {
                UuidGenerationMethod::GenRandomUuid => Some("generate_uuid()".to_string()),
                UuidGenerationMethod::UuidGenerateV4 => Some("uuidGenerateV4()".to_string()),
            },
            ColumnDefault::CurrentTimestamp | ColumnDefault::CurrentDate => {
                Some("now()".to_string())
            }
            ColumnDefault::Text(value) | ColumnDefault::VarChar(value) => {
                Some(format!("\"{value}\""))
            }
            ColumnDefault::Boolean(value) => Some(format!("{value}")),
            ColumnDefault::Number(value) => Some(value.to_string()),
            ColumnDefault::Function(value) => Some(value.clone()),
            ColumnDefault::Enum(value) => Some(value.to_string()),
            ColumnDefault::Autoincrement(autoincrement) => match autoincrement {
                ColumnAutoincrement::Serial => Some("autoIncrement()".to_string()),
                ColumnAutoincrement::Sequence { name } => Some(format!(
                    "autoIncrement(\"{}.{}\")",
                    name.schema.as_deref().unwrap_or("public"),
                    name.name
                )),
                ColumnAutoincrement::Identity { .. } => {
                    todo!()
                }
            },
            ColumnDefault::Date(value) => Some(format!("\"{value}\"")),
            ColumnDefault::Time(value) => Some(format!("\"{value}\"")),
            ColumnDefault::DateTime(value) => Some(format!("\"{value}\"")),
            ColumnDefault::Json(value) => Some(format!("\"{value}\"")),
            ColumnDefault::UuidLiteral(value) => Some(format!("\"{value}\"")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColumnReferenceSpec {
    pub foreign_table_name: SchemaObjectName,
    pub foreign_pk_column_name: String,
    pub foreign_pk_type: Box<dyn PhysicalColumnType>,
    pub group_name: String,
}

impl PartialEq for ColumnReferenceSpec {
    fn eq(&self, other: &Self) -> bool {
        self.foreign_table_name == other.foreign_table_name
            && self.foreign_pk_column_name == other.foreign_pk_column_name
            && self.foreign_pk_type.equals(other.foreign_pk_type.as_ref())
            && self.group_name == other.group_name
    }
}

impl Eq for ColumnReferenceSpec {}

impl DebugPrintTo for ColumnReferenceSpec {
    fn debug_print_to<W: std::io::Write>(
        &self,
        writer: &mut W,
        indent: usize,
    ) -> std::io::Result<()> {
        let indent_str = " ".repeat(indent);
        writeln!(
            writer,
            "{}ColumnReference: FK -> {}.{} (type: {})",
            indent_str,
            self.foreign_table_name.fully_qualified_name(),
            self.foreign_pk_column_name,
            self.foreign_pk_type.type_string()
        )
    }
}

const COLUMNS_TYPE_QUERY: &str = "
  SELECT pg_class.relname as table_name, attname as column_name, format_type(atttypid, atttypmod), attndims, attnotnull FROM pg_attribute 
    LEFT JOIN pg_class ON pg_attribute.attrelid = pg_class.oid 
    LEFT JOIN pg_namespace ON pg_class.relnamespace = pg_namespace.oid
  WHERE attnum > 0 AND attisdropped = false AND pg_namespace.nspname = $1";

const COLUMNS_DEFAULT_QUERY: &str = r#"
    SELECT
        t.table_name,
        c.column_name,
        c.column_default,
        CASE
            WHEN c.is_identity = 'YES' THEN true
            ELSE false
        END AS is_identity,
        c.identity_generation,
        CASE 
            WHEN c.column_default LIKE 'nextval%' THEN 
                CASE 
                    WHEN substring(c.column_default from '''(.*)''') LIKE '%.%' THEN
                        split_part(substring(c.column_default from '''(.*)'''), '.', 1)
                    ELSE
                        t.table_schema -- Default to table's schema if no schema in sequence
                END
            WHEN c.is_identity = 'YES' THEN
                split_part(pg_get_serial_sequence(t.table_schema || '.' || t.table_name, c.column_name), '.', 1)
            ELSE NULL
        END AS sequence_schema,
        CASE 
            WHEN c.column_default LIKE 'nextval%' THEN 
                CASE 
                    WHEN substring(c.column_default from '''(.*)''') LIKE '%.%' THEN
                        split_part(substring(c.column_default from '''(.*)'''), '.', 2)
                    ELSE
                        regexp_replace(substring(c.column_default from '''(.*)'''), '''::.*$', '')
                END
            WHEN c.is_identity = 'YES' THEN
                split_part(pg_get_serial_sequence(t.table_schema || '.' || t.table_name, c.column_name), '.', 2)
            ELSE NULL
        END AS sequence_name,
        CASE 
            WHEN c.is_identity = 'YES' THEN true
            WHEN c.column_default LIKE 'nextval%' THEN true
            ELSE false
        END AS is_autoincrement
    FROM 
        information_schema.tables t
    JOIN 
        information_schema.columns c 
        ON t.table_name = c.table_name 
        AND t.table_schema = c.table_schema
    WHERE 
        t.table_schema = $1
        AND t.table_type = 'BASE TABLE'
    ORDER BY 
        t.table_name, 
        c.ordinal_position;
"#;

const TEXT_TYPE_CAST_PREFIX: &str = "'::text";
const CHARACTER_VARYING_TYPE_CAST_PREFIX: &str = "'::character varying";

impl ColumnSpec {
    /// Creates a new column specification from an SQL column.
    ///
    /// If the column references another table's column, the column's type can be specified with
    /// `explicit_type`.
    pub async fn from_live_db(
        table_name: &SchemaObjectName,
        column_name: &str,
        is_pk: bool,
        explicit_type: Option<Box<dyn PhysicalColumnType>>,
        unique_constraints: Vec<String>,
        column_attributes: &HashMap<SchemaObjectName, HashMap<String, ColumnAttribute>>,
    ) -> Result<WithIssues<Option<ColumnSpec>>, DatabaseError> {
        let table_attributes = column_attributes
            .get(table_name)
            .ok_or(DatabaseError::Generic(format!(
                "Table `{}` not found",
                table_name.fully_qualified_name()
            )))?;

        let ColumnAttribute {
            default_value,
            not_null,
            db_type,
        } = table_attributes
            .get(column_name)
            .ok_or(DatabaseError::Generic(format!(
                "Column `{}` not found in table `{}`",
                column_name,
                table_name.fully_qualified_name()
            )))?;

        let db_type = explicit_type.or(db_type.clone());

        Ok(WithIssues {
            value: db_type.map(|typ| ColumnSpec {
                name: column_name.to_owned(),
                typ,
                reference_specs: None,
                is_pk,
                is_nullable: !not_null,
                unique_constraints,
                default_value: default_value.clone(),
            }),
            issues: vec![],
        })
    }

    /// Converts the column specification to SQL statements.
    pub(super) fn to_sql(&self, attach_pk_column_to_column_stmt: bool) -> SchemaStatement {
        let SchemaStatement {
            statement,
            post_statements,
            ..
        } = self.typ.to_sql(self.default_value.as_ref());
        let pk_str = if self.is_pk && attach_pk_column_to_column_stmt {
            " PRIMARY KEY"
        } else {
            ""
        };
        let not_null_str = if !self.is_nullable && !self.is_pk {
            // primary keys are implied to be not null
            " NOT NULL"
        } else {
            ""
        };
        let default_value_part = self
            .default_value
            .as_ref()
            .and_then(|default_value| default_value.to_sql().map(|s| format!(" DEFAULT {s}")))
            .unwrap_or_default();

        SchemaStatement {
            statement: format!(
                "\"{}\" {}{}{}{}",
                self.name, statement, pk_str, not_null_str, default_value_part
            ),
            pre_statements: vec![],
            post_statements,
        }
    }

    pub fn diff<'a>(
        &'a self,
        new: &'a Self,
        self_table: &'a TableSpec,
        new_table: &'a TableSpec,
    ) -> Vec<SchemaOp<'a>> {
        let mut changes = vec![];
        let table_name_same = self_table.sql_name() == new_table.sql_name();
        let column_name_same = self.name == new.name;
        let type_same = self.typ.equals(new.typ.as_ref());
        let reference_specs_same = self.reference_specs == new.reference_specs;
        let is_pk_same = self.is_pk == new.is_pk;
        let is_nullable_same = self.is_nullable == new.is_nullable;
        let default_value_same = self.default_value == new.default_value;

        if !(table_name_same && column_name_same) {
            panic!("Diffing columns must have the same table name and column name");
        }

        // If the column type differs only in reference type, that is taken care by table-level migration
        if (!type_same && !self.differs_only_in_reference_column(new))
            || (!reference_specs_same && !self.differs_only_in_reference_column(new))
            || !is_pk_same
        {
            changes.push(SchemaOp::DeleteColumn {
                table: self_table,
                column: self,
            });
            changes.push(SchemaOp::CreateColumn {
                table: new_table,
                column: new,
            })
        } else if !is_nullable_same {
            if new.is_nullable && !self.is_nullable {
                // drop NOT NULL constraint
                changes.push(SchemaOp::UnsetNotNull {
                    table: self_table,
                    column: self,
                })
            } else {
                // add NOT NULL constraint
                changes.push(SchemaOp::SetNotNull {
                    table: self_table,
                    column: self,
                })
            }
        } else if !default_value_same {
            match new
                .default_value
                .as_ref()
                .and_then(|default_value| default_value.to_sql())
            {
                Some(default_value) => {
                    changes.push(SchemaOp::SetColumnDefaultValue {
                        table: new_table,
                        column: new,
                        default_value,
                    });
                }
                None => {
                    changes.push(SchemaOp::UnsetColumnDefaultValue {
                        table: new_table,
                        column: new,
                    });
                }
            }
        }

        changes
    }

    pub(crate) fn from_physical(column: PhysicalColumn, database: &Database) -> ColumnSpec {
        let (typ, reference_specs) = {
            let reference_specs = column.column_references.map(|column_references| {
                column_references
                    .iter()
                    .map(|column_reference| {
                        let foreign_column_id = column_reference.foreign_column_id;
                        let foreign_column = foreign_column_id.get_column(database);
                        let foreign_table = database.get_table(foreign_column.table_id);

                        ColumnReferenceSpec {
                            foreign_table_name: foreign_table.name.clone(),
                            foreign_pk_column_name: foreign_column.name.clone(),
                            foreign_pk_type: foreign_column.typ.clone(),
                            group_name: column_reference.group_name.clone(),
                        }
                    })
                    .collect()
            });

            (column.typ, reference_specs)
        };

        ColumnSpec {
            name: column.name,
            typ,
            is_pk: column.is_pk,
            is_nullable: column.is_nullable,
            unique_constraints: column.unique_constraints,
            default_value: column.default_value,
            reference_specs,
        }
    }

    fn differs_only_in_reference_column(&self, new: &Self) -> bool {
        match (&self.reference_specs, &new.reference_specs) {
            (Some(_), Some(_)) => {
                (self.reference_specs != new.reference_specs) && {
                    Self {
                        typ: Box::new(IntColumnType { bits: IntBits::_16 }),
                        reference_specs: None,
                        ..self.clone()
                    } == Self {
                        typ: Box::new(IntColumnType { bits: IntBits::_16 }),
                        reference_specs: None,
                        ..new.clone()
                    }
                }
            }
            _ => false,
        }
    }

    pub fn with_table_renamed(
        mut self,
        old_name: &SchemaObjectName,
        new_name: &SchemaObjectName,
    ) -> Self {
        if let Some(reference_specs) = &mut self.reference_specs {
            for reference_spec in reference_specs {
                if reference_spec.foreign_table_name == *old_name {
                    reference_spec.foreign_table_name = new_name.clone();
                }
            }
        }

        self
    }

    /// Compute column attributes from all tables in the given schema
    pub async fn query_column_attributes(
        client: &DatabaseClient,
        schema_name: &str,
        enums: &Vec<EnumSpec>,
        issues: &mut Vec<Issue>,
    ) -> Result<HashMap<SchemaObjectName, HashMap<String, ColumnAttribute>>, DatabaseError> {
        let mut map = HashMap::new();

        for row in client.query(COLUMNS_TYPE_QUERY, &[&schema_name]).await? {
            let table_name: String = row.get("table_name");
            let column_name: String = row.get("column_name");
            let not_null: bool = row.get("attnotnull");

            let table_name = SchemaObjectName::new_with_schema_name(table_name, schema_name);

            let db_type = {
                let mut sql_type: String = row.get("format_type");

                let dims = {
                    // depending on the version of postgres, the type of `attndims` is either `i16`
                    // or `i32` (postgres type is `int2`` or `int4``), so try both
                    let dims: Result<i32, _> = row.try_get("attndims");

                    match dims {
                        Ok(dims) => dims,
                        Err(_) => {
                            let dims: i16 = row.get("attndims");
                            dims as i32
                        }
                    }
                };

                // When querying array types, the number of dimensions is not correctly shown
                // e.g. a column declared as `INT[][][]` will be shown as `INT[]`
                // So we manually query how many dimensions the column has and append `[]` to
                // the type
                sql_type += &"[]".repeat(if dims == 0 { 0 } else { (dims - 1) as usize });
                match physical_column_type_from_string(&sql_type, enums) {
                    Ok(t) => Some(t),
                    Err(e) => {
                        issues.push(Issue::Warning(format!(
                            "skipped column `{}.{column_name}` ({e})",
                            table_name.fully_qualified_name()
                        )));
                        None
                    }
                }
            };

            let table_attributes = map.entry(table_name).or_insert_with(HashMap::new);

            table_attributes.insert(
                column_name,
                ColumnAttribute {
                    db_type,
                    not_null,
                    default_value: None,
                },
            );
        }

        for row in client.query(COLUMNS_DEFAULT_QUERY, &[&schema_name]).await? {
            let table_name: String = row.get("table_name");
            let column_name: String = row.get("column_name");
            let is_autoincrement = row.get("is_autoincrement");

            let table_name = SchemaObjectName::new_with_schema_name(table_name, schema_name);

            // If this column is autoIncrement, then default value will be populated
            // with an invocation of nextval(). Thus, we need to clear it to normalize the column
            let default_value = if is_autoincrement {
                let is_identity = row.get("is_identity");
                let sequence_schema: String = row.get("sequence_schema");
                let sequence_name: String = row.get("sequence_name");

                let autoincrement = if is_identity {
                    let generation_str: Option<String> = row.get("identity_generation");
                    let generation = match generation_str.as_deref() {
                        Some("ALWAYS") => IdentityGeneration::Always,
                        Some("DEFAULT") => IdentityGeneration::Default,
                        _ => {
                            return Err(DatabaseError::Validation(format!(
                                "unknown identity generation {generation_str:?}"
                            )));
                        }
                    };
                    ColumnAutoincrement::Identity { generation }
                } else {
                    let serial_sequence_name =
                        ColumnAutoincrement::serial_sequence_name(&table_name, &column_name);
                    let from_db_sequence_name =
                        SchemaObjectName::new_with_schema_name(sequence_name, sequence_schema);

                    if serial_sequence_name == from_db_sequence_name {
                        ColumnAutoincrement::Serial
                    } else {
                        ColumnAutoincrement::Sequence {
                            name: from_db_sequence_name,
                        }
                    }
                };

                Ok(Some(ColumnDefault::Autoincrement(autoincrement)))
            } else {
                let default_value: Option<String> = row.get("column_default");
                default_value
                    .map(|default_value| {
                        ColumnDefault::from_sql(
                            default_value,
                            map.get(&table_name).and_then(|table_attributes| {
                                table_attributes
                                    .get(&column_name)
                                    .and_then(|info| info.db_type.as_ref().map(|t| t.as_ref()))
                            }),
                        )
                    })
                    .transpose()
            }?;

            let table_attributes = map.entry(table_name).or_insert_with(HashMap::new);

            table_attributes.entry(column_name).and_modify(|info| {
                info.default_value = default_value;
            });
        }

        Ok(map)
    }
}

#[derive(Debug)]
pub struct ColumnAttribute {
    pub default_value: Option<ColumnDefault>,
    pub db_type: Option<Box<dyn PhysicalColumnType>>,
    pub not_null: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum ColumnAutoincrement {
    Serial, // Maps to `SERIAL` in postgres (sequence is `{schema}.{table}_{column}_id_seq`)
    Sequence { name: SchemaObjectName },
    Identity { generation: IdentityGeneration },
}

impl ColumnAutoincrement {
    pub fn serial_sequence_name(
        table_name: &SchemaObjectName,
        column_name: &str,
    ) -> SchemaObjectName {
        SchemaObjectName {
            name: format!("{}_{}_seq", table_name.name, column_name),
            schema: table_name.schema.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum IdentityGeneration {
    Always,
    Default,
}

/// Create a physical column type from the SQL type string. This is used to reverse-engineer
/// a database schema to a Exograph model.
pub fn physical_column_type_from_string(
    s: &str,
    enums: &Vec<EnumSpec>,
) -> Result<Box<dyn PhysicalColumnType>, DatabaseError> {
    let s_upper = s.to_uppercase();

    // Check for vector type first
    let vector_re = Regex::new(r"VECTOR\((\d+)\)").unwrap();
    if let Some(captures) = vector_re.captures(&s_upper) {
        let size = captures.get(1).unwrap().as_str().parse().unwrap();
        return Ok(Box::new(VectorColumnType { size }));
    }

    // Check for array types
    match s_upper.find('[') {
        // If the type contains `[`, then it's an array type
        Some(idx) => {
            let db_type = &s_upper[..idx]; // The underlying data type (e.g. `INT` in `INT[][]`)
            let mut dims = &s_upper[idx..]; // The array brackets (e.g. `[][]` in `INT[][]`)

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

            // Parse the base type and wrap in Array
            let base_physical = physical_column_type_from_string(db_type, enums)?;

            let mut array_type: Box<dyn PhysicalColumnType> =
                Box::new(ArrayColumnType { typ: base_physical });
            for _ in 0..count - 1 {
                array_type = Box::new(ArrayColumnType { typ: array_type });
            }
            Ok(array_type)
        }
        None => {
            // Check if it's an enum type
            let enum_type = enums.iter().find(|enum_spec| {
                let enum_spec_name = match &enum_spec.name.schema {
                    Some(schema) => format!("{}.{}", schema, enum_spec.name.name),
                    None => enum_spec.name.name.clone(),
                };
                enum_spec_name.to_uppercase() == s_upper
            });

            if let Some(enum_type) = enum_type {
                Ok(Box::new(EnumColumnType {
                    enum_name: enum_type.name.clone(),
                }))
            } else {
                // Try to parse as a regular PhysicalColumnType
                crate::sql::physical_column_type::physical_column_type_from_string_boxed(s)
            }
        }
    }
}
