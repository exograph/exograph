// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{cmp::Ordering, hash::Hash};

use serde::{Deserialize, Serialize};

/// A name of a table/enum/sequence along with its schema.
#[derive(Serialize, Deserialize, Debug, Eq, Clone)]
pub struct SchemaObjectName {
    pub name: String,
    /// Default is "public".
    pub schema: Option<String>,
}

impl PartialOrd for SchemaObjectName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SchemaObjectName {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.name, self.schema.as_deref()).cmp(&(&other.name, other.schema.as_deref()))
    }
}

impl PartialEq for SchemaObjectName {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && match (self.schema.as_deref(), other.schema.as_deref()) {
                (Some(s1), Some(s2)) => s1 == s2,
                (None, None) | (Some("public"), None) | (None, Some("public")) => true,
                _ => false,
            }
    }
}

impl Hash for SchemaObjectName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        match &self.schema {
            Some(schema) if schema != "public" => schema.hash(state),
            _ => (),
        }
    }
}

impl SchemaObjectName {
    pub fn new(name: impl Into<String>, schema: Option<&str>) -> Self {
        Self {
            name: name.into(),
            schema: match schema {
                Some(schema) if schema != "public" => Some(schema.to_string()),
                _ => None,
            },
        }
    }

    pub fn new_with_schema_name(name: impl Into<String>, schema_name: impl Into<String>) -> Self {
        let schema_name = schema_name.into();

        Self {
            name: name.into(),
            schema: match schema_name.as_str() {
                "public" => None,
                _ => Some(schema_name),
            },
        }
    }

    pub fn fully_qualified_name(&self) -> String {
        self.fully_qualified_name_with_sep(".")
    }

    pub fn fully_qualified_name_with_sep(&self, sep: &str) -> String {
        match &self.schema {
            Some(schema) => format!("{}{}{}", schema, sep, self.name),
            None => self.name.to_owned(),
        }
    }

    pub(crate) fn synthetic_name(&self) -> String {
        match &self.schema {
            Some(schema) => format!("{}#{}", schema, self.name),
            None => self.name.to_owned(),
        }
    }

    pub fn sql_name(&self) -> String {
        match self.schema {
            Some(ref schema) => format!("\"{}\".\"{}\"", schema, self.name),
            None => format!("\"{}\"", self.name),
        }
    }

    pub fn schema_name(&self) -> String {
        match self.schema {
            Some(ref schema) => schema.to_string(),
            None => "public".to_string(),
        }
    }
}
