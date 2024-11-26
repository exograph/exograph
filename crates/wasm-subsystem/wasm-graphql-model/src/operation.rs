// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::ops::Deref;

use serde::{Deserialize, Serialize};
use subsystem_model_util::operation::{ModuleMutation, ModuleQuery};

#[derive(Serialize, Deserialize, Debug)]
pub struct WasmQuery(pub ModuleQuery);

impl Deref for WasmQuery {
    type Target = ModuleQuery;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WasmMutation(pub ModuleMutation);

impl Deref for WasmMutation {
    type Target = ModuleMutation;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
