// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.
import { rust_impl, async_rust_impl } from "test:through_rust";

export function syncUsingRegisteredFunction(value) {
  return rust_impl(value)
}

export function asyncUsingRegisteredFunction(value) {
  return async_rust_impl(value)
}
