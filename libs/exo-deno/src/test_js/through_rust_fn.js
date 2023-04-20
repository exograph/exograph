// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

export function syncUsingRegisteredFunction(value) {
  return Deno[Deno.internal].core.ops.rust_impl(value)
}

export async function asyncUsingRegisteredFunction(value) {
  return Deno[Deno.internal].core.opAsync("async_rust_impl", value)
}
