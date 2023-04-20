// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Dynamic loader for deno-resolver.
use core_plugin_interface::interface::SubsystemLoader;
use deno_resolver::DenoSubsystemLoader;

// See comments in `postgres-resolver-dynamic/src/lib.rs`.
core_plugin_interface::export_subsystem_loader!(DenoSubsystemLoader {});
