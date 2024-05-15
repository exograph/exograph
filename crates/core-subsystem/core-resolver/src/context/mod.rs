// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

mod context_extractor;
mod error;
mod overridden_context;
pub mod provider;
mod request;
mod request_context;
mod user_request_context;

pub use provider::jwt::JwtAuthenticator;

pub use error::ContextExtractionError;
pub use request::Request;
pub use request_context::RequestContext;

#[cfg(feature = "test-context")]
pub use context_extractor::TestRequestContext;
