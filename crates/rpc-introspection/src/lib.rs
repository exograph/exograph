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
//! - Validating RPC parameters against schemas
//!
//! # Example
//!
//! ```
//! use rpc_introspection::schema::{RpcSchema, RpcMethod, RpcTypeSchema};
//! use rpc_introspection::conversion::to_openrpc;
//!
//! let mut schema = RpcSchema::new();
//! schema.add_method(RpcMethod::new(
//!     "hello".to_string(),
//!     RpcTypeSchema::scalar("String"),
//! ));
//!
//! let openrpc = to_openrpc(&schema, "My API", "1.0.0");
//! ```

pub mod conversion;
pub mod openrpc;
pub mod schema;
pub mod validation;

// Re-export commonly used types
pub use conversion::to_openrpc;
pub use openrpc::OpenRpcDocument;
pub use schema::{RpcMethod, RpcParameter, RpcSchema, RpcTypeSchema};
pub use validation::{ValidationError, validate_with_constraints};
