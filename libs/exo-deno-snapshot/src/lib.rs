// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! V8 snapshot used by exo-deno for faster runtime initialization.
//!
//! This crate is separated from exo-deno to isolate the build script complexity.
//! The build script creates a V8 snapshot by calling deno_runtime::snapshot::create_runtime_snapshot(),
//! which has hundreds of dependencies (SWC, V8, etc.). By separating this into its own crate,
//! the build script executable is built separately and can be cached, avoiding MSVC linker
//! PDB complexity errors on Windows.

static RUNTIME_SNAPSHOT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/RUNTIME_SNAPSHOT.bin"));

pub fn snapshot() -> &'static [u8] {
    RUNTIME_SNAPSHOT
}
