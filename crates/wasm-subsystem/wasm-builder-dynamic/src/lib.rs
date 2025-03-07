// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Dynamic loader for wasm-builder.
use core_plugin_interface::interface::SubsystemBuilder;
use wasm_builder::WasmSubsystemBuilder;

// See comments in `postgres-graphqlresolver-dynamic/src/lib.rs`.
core_plugin_interface::export_subsystem_builder!(WasmSubsystemBuilder::default());
