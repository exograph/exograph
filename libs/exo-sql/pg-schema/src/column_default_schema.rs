// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::column_default::{ColumnAutoincrement, ColumnDefault, UuidGenerationMethod};

/// PostgreSQL DDL generation for column defaults.
pub trait ColumnDefaultSchema {
    fn to_schema(&self) -> Option<String>;
}

impl ColumnDefaultSchema for ColumnDefault {
    fn to_schema(&self) -> Option<String> {
        match self {
            ColumnDefault::Uuid(method) => match method {
                UuidGenerationMethod::Random => Some("gen_random_uuid()".to_string()),
                UuidGenerationMethod::V4 => Some("uuid_generate_v4()".to_string()),
                UuidGenerationMethod::V7 => Some("uuidv7()".to_string()),
            },
            ColumnDefault::CurrentTimestamp => Some("now()".to_string()),
            ColumnDefault::CurrentDate => Some("now()".to_string()),
            ColumnDefault::Text(value) => Some(format!("'{value}'::text")),
            ColumnDefault::VarChar(value) => Some(format!("'{value}'::character varying")),
            ColumnDefault::Boolean(value) => Some(format!("{value}")),
            ColumnDefault::Number(value) => Some(value.to_string()),
            ColumnDefault::Int(value) => Some(value.to_string()),
            ColumnDefault::Float(value) => Some(value.to_string()),
            ColumnDefault::Decimal(value) => Some(value.to_string()),
            ColumnDefault::Function(value) => Some(value.clone()),
            ColumnDefault::Enum(value) => Some(format!("'{value}'")),
            ColumnDefault::Autoincrement(autoincrement) => match autoincrement {
                ColumnAutoincrement::Serial => None,
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
}
