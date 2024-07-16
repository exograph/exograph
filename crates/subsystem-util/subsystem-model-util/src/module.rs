// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

use crate::{access::Access, types::ModuleOperationReturnType};

use super::{
    operation::{ModuleMutation, ModuleQuery},
    types::ModuleType,
};
use core_model::{mapped_arena::SerializableSlabIndex, types::FieldType};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleMethod {
    pub name: String,
    pub script: SerializableSlabIndex<Script>,
    pub operation_kind: ModuleMethodType,
    pub is_exported: bool,
    pub arguments: Vec<Argument>,
    pub access: Access,
    pub return_type: ModuleOperationReturnType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Script {
    pub path: String,
    pub script: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Argument {
    pub name: String,
    pub type_id: FieldType<SerializableSlabIndex<ModuleType>>,
    pub is_injected: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ModuleMethodType {
    Query(SerializableSlabIndex<ModuleQuery>),
    Mutation(SerializableSlabIndex<ModuleMutation>),
}
