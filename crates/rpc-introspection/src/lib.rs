// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! RPC Introspection crate for Exograph.
//!
//! This crate provides types and utilities for:
//! - Representing RPC method schemas
//! - Converting schemas to OpenRPC format
//!
//! # Example
//!
//! ```ignore
//! use rpc_introspection::schema::{RpcSchema, RpcMethod, RpcTypeSchema};
//! use rpc_introspection::conversion::{to_rpc_document, SchemaGeneration};
//!
//! let mut schema = RpcSchema::new();
//! schema.add_method(RpcMethod::new(
//!     "hello".to_string(),
//!     RpcTypeSchema::scalar("String"),
//! ));
//!
//! let doc = to_rpc_document(&schema, SchemaGeneration::OpenRpc);
//! ```

pub mod conversion;
pub mod openrpc;
pub mod rpc_schema_doc;
pub mod schema;
pub mod validation;

// Re-export commonly used types
pub use conversion::{SchemaGeneration, to_rpc_document};
pub use openrpc::OpenRpcDocument;
pub use rpc_schema_doc::RpcDocument;
pub use schema::{RpcMethod, RpcParameter, RpcSchema, RpcTypeSchema};
pub use validation::RpcValidationError;
