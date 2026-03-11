// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! OpenRPC specification types.
//!
//! This module defines types specific to the OpenRPC 1.3.2 specification.
//! See: https://spec.open-rpc.org/
//!
//! Generic RPC schema types shared with MCP are in [`crate::rpc_schema_doc`].

use serde::{Deserialize, Serialize};

use crate::rpc_schema_doc::RpcDocument;

/// The root OpenRPC document, wrapping an `RpcDocument` with OpenRPC-specific metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRpcDocument {
    /// The OpenRPC specification version (e.g., "1.3.2")
    pub openrpc: String,
    /// Metadata about the API
    pub info: InfoObject,
    /// The schema document
    #[serde(flatten)]
    pub document: RpcDocument,
}

impl OpenRpcDocument {
    /// Create a new OpenRPC document with the given title and version
    pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            openrpc: "1.3.2".to_string(),
            info: InfoObject {
                title: title.into(),
                version: version.into(),
                description: None,
            },
            document: RpcDocument::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.info.description = Some(description.into());
        self
    }

    pub fn with_document(mut self, document: RpcDocument) -> Self {
        self.document = document;
        self
    }
}

/// Metadata about the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoObject {
    /// The title of the API
    pub title: String,
    /// The version of the API (not the OpenRPC spec version)
    pub version: String,
    /// A description of the API
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc_schema_doc::{ContentDescriptor, JsonSchemaInline, MethodObject};

    #[test]
    fn openrpc_document_serialization() {
        let rpc_doc = RpcDocument::new().with_method(
            MethodObject::new(
                "get_item",
                ContentDescriptor::new("result", JsonSchemaInline::object().into()),
            )
            .with_param(
                ContentDescriptor::new("id", JsonSchemaInline::integer().into()).required(),
            ),
        );
        let doc = OpenRpcDocument::new("Test API", "1.0.0")
            .with_description("A test API")
            .with_document(rpc_doc);

        let json = serde_json::to_value(&doc).unwrap();
        assert_eq!(json["openrpc"], "1.3.2");
        assert_eq!(json["info"]["title"], "Test API");
        assert_eq!(json["info"]["version"], "1.0.0");
        assert_eq!(json["info"]["description"], "A test API");
        assert_eq!(json["methods"][0]["name"], "get_item");
        assert_eq!(json["methods"][0]["params"][0]["name"], "id");
        assert_eq!(json["methods"][0]["params"][0]["required"], true);
    }
}
