// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::SchemaObjectName;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ColumnDefault {
    Uuid(UuidGenerationMethod),
    CurrentTimestamp,
    CurrentDate,
    Text(String),
    VarChar(String),
    Boolean(bool),
    Number(String),
    Int(String),
    Float(String),
    Decimal(String),
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
    Random,
    V4,
    V7,
}

impl ColumnDefault {
    pub fn is_autoincrement(&self) -> bool {
        matches!(self, ColumnDefault::Autoincrement(_))
    }

    pub fn to_model(&self) -> Option<String> {
        match self {
            ColumnDefault::Uuid(method) => match method {
                UuidGenerationMethod::Random => Some("generate_uuid()".to_string()),
                UuidGenerationMethod::V4 => Some("uuidGenerateV4()".to_string()),
                UuidGenerationMethod::V7 => Some("uuidGenerateV7()".to_string()),
            },
            ColumnDefault::CurrentTimestamp | ColumnDefault::CurrentDate => {
                Some("now()".to_string())
            }
            ColumnDefault::Text(value) | ColumnDefault::VarChar(value) => {
                Some(format!("\"{value}\""))
            }
            ColumnDefault::Boolean(value) => Some(format!("{value}")),
            ColumnDefault::Number(value) => Some(format!("\"{value}\"")),
            ColumnDefault::Int(value) => Some(value.to_string()),
            ColumnDefault::Float(value) => Some(value.to_string()),
            ColumnDefault::Decimal(value) => Some(format!("\"{value}\"")),
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
