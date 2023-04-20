// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

use super::module::{Argument, Script};
use core_model::mapped_arena::SerializableSlabIndex;
use core_plugin_shared::interception::InterceptorKind;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Interceptor {
    pub module_name: String,
    pub method_name: String,
    pub script: SerializableSlabIndex<Script>,
    pub interceptor_kind: InterceptorKind,
    pub arguments: Vec<Argument>,
}
