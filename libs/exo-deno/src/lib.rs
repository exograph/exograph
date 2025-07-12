// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub mod deno_executor;
pub mod deno_executor_pool;
pub mod deno_module;
/// This code has no concept of Exograph.
///
/// Module to encapsulate the logic creating a Deno module that supports
/// embedding.
///
pub mod error;

use std::sync::atomic::{AtomicBool, Ordering};

pub use deno_error;
pub use deno_executor_pool::DenoExecutorPool;
pub use deno_module::{Arg, DenoModule, UserCode};

mod deno_actor;
mod embedded_module_loader;

pub use deno_core;

static RUNTIME_SNAPSHOT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/RUNTIME_SNAPSHOT.bin"));

pub(crate) fn deno_snapshot() -> &'static [u8] {
    RUNTIME_SNAPSHOT
}

#[cfg(test)]
use ctor::ctor;

#[cfg(test)]
#[ctor]
// Make sure deno runtime is initialized in the main thread in test executables.
fn initialize_for_tests() {
    initialize();
}

static INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn initialize() {
    if INITIALIZED.load(Ordering::Relaxed) {
        return;
    }
    INITIALIZED.store(true, Ordering::Relaxed);

    use deno_runtime::deno_tls::rustls;

    deno_core::JsRuntime::init_platform(None, true);
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();
}
