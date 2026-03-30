// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub mod deno_execution_error;
pub mod exo_execution;
pub mod exograph_ops;

pub use exo_execution::exo_config;

use exo_deno::DenoExecutorPool;

/// The Deno executor pool type parameterized with Exograph's callback types.
pub type ExoDenoExecutorPool = DenoExecutorPool<
    Option<exograph_ops::InterceptedOperationInfo>,
    exo_execution::RequestFromDenoMessage,
    exo_execution::ExographMethodResponse,
>;
