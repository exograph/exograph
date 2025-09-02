// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub(crate) mod execution;

pub use execution::introspection::{
    create_introspection_deno_module, execute_introspection_deno_function, get_introspection_query,
    schema_sdl,
};

#[cfg(test)]
use ctor::ctor;

#[cfg(test)]
#[ctor]
// Make sure deno runtime is initialized in the main thread in test executables.
fn initialize_for_tests() {
    exo_deno::initialize();
}
