// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{Column, Database};

use super::{
    physical_column::{PhysicalColumn, PhysicalColumnType},
    ExpressionBuilder, SQLBuilder,
};

/// A JSON object corresponding to the Postgres' `json_build_object` function.
#[derive(Debug, PartialEq)]
pub struct JsonObject(pub Vec<JsonObjectElement>);

/// A key-value pair in a JSON object.
#[derive(Debug, PartialEq)]
pub struct JsonObjectElement {
    pub key: String,
    pub value: Column,
}

impl JsonObjectElement {
    pub fn new(key: String, value: Column) -> Self {
        Self { key, value }
    }
}

impl ExpressionBuilder for JsonObject {
    /// Build expression of the form `json_build_object(<comma-separated-elements>)`.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("json_build_object(");
        builder.push_elems(database, &self.0, ", ");
        builder.push(')');
    }
}

impl ExpressionBuilder for JsonObjectElement {
    /// Build an SQL query for an element in a JSON object. The SQL expression will be `'<key>',
    /// <value>`, where `<value>` is the SQL expression for the value of the JSON object element. The
    /// value of the JSON object element is encoded as base64 if it is a blob, and as text if it is a
    /// numeric.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("'");
        builder.push_str(&self.key);
        builder.push_str("', ");

        if let Column::Physical(column_id) = self.value {
            let PhysicalColumn { typ, .. } = column_id.get_column(database);
            match &typ {
                // encode blob fields in JSON objects as base64
                // PostgreSQL inserts newlines into encoded base64 every 76 characters when in aligned mode
                // need to filter out using translate(...) function
                PhysicalColumnType::Blob => {
                    builder.push_str("translate(encode(");
                    self.value.build(database, builder);
                    builder.push_str(", \'base64\'), E'\\n', '')");
                }

                // numerics must be outputted as text to avoid any loss in precision
                PhysicalColumnType::Numeric { .. } => {
                    self.value.build(database, builder);
                    builder.push_str("::text");
                }

                _ => self.value.build(database, builder),
            }
        } else {
            self.value.build(database, builder)
        }
    }
}
